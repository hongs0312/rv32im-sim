use crate::hardware::clocked::Clocked;
use crate::hardware::soc::on_chip_bus::SystemBus;
use crate::hardware::soc::types::OpStatus;

use super::{Core, Trap};

use super::components::alu::{AluInputs, AluNextState};

use super::control::{
    branch_controller::BranchController,
    hazard_detection_unit::HazardDetectionUnit,
    pipeline_control::{PipelineControl, StallSignals},
};

use super::pipeline_registers::{
    ExMemRegister, LegacyNextState, MemWbRegister, PipelineState, StageResults,
};

use super::pipeline_stage::{
    execute, instruction_decode, instruction_fetch, memory_access, write_back,
};

#[derive(Default)]
struct ExOutput {
    register: ExMemRegister,
    busy: bool,
    branch_target: Option<u32>,
}

type PendingTrap = Option<(Trap, u32)>;

impl Core {
    /// 현재 호환 인터페이스에서 한 사이클을 실행한다.
    ///
    /// ALU와 파이프라인 래치는 마지막에 한 번 반영한다.
    /// 기존 단계 내부의 CSR·캐시·MMIO 부작용은 아직 유지한다.
    pub fn pipeline_step(&mut self, bus: &mut SystemBus, inject_nop: bool) {
        let next = self.run_stages(bus, inject_nop);
        self.commit_cycle(bus, next);
    }

    fn run_stages(&mut self, bus: &mut SystemBus, inject_nop: bool) -> LegacyNextState {
        let current = self.pipeline_state();

        // EX가 버블이거나 조기 반환해도 기존 ALU 작업은 진행한다.
        let (_, default_alu_next) = self.alu.evaluate(&AluInputs {
            request: None,
            result_ready: false,
            flush: false,
        });

        let mut next_alu = Some(default_alu_next);
        let mut trap: PendingTrap = None;

        // 1. WB
        write_back::execute(self, current.mem_wb);

        // 2. MEM
        let (mem_busy, mem_wb) = self.run_mem(bus, &mut trap);

        // MEM 동기 예외가 있으면 인터럽트가 가리지 않도록 한다.
        if !mem_busy && trap.is_none() {
            if let Some(next) = self.run_interrupt(mem_wb) {
                return next;
            }
        }

        // 3. EX
        let ex = self.run_ex(
            &mem_wb,
            !mem_busy && trap.is_none(),
            &mut next_alu,
            &mut trap,
        );

        // 4. ID
        let load_use = HazardDetectionUnit::check_load_use(
            current.id_ex.control.mem_read,
            current.id_ex.rd,
            current.if_id.instruction,
        );

        let id_ex = instruction_decode::execute(self, current.if_id);

        // 5. IF
        let (if_busy, if_id) = match instruction_fetch::execute(self, bus, inject_nop) {
            Ok(OpStatus::Complete(register)) => (false, register),

            Ok(OpStatus::Busy) => (true, Default::default()),

            Err(error) => {
                if trap.is_none() {
                    trap = Some((error, current.pc));
                }

                (false, Default::default())
            }
        };

        // 기존 예외 처리 방식을 유지한다.
        // IF 예외까지 명령 슬롯으로 운반하는 개선은 별도 작업이다.
        if trap.is_some() {
            next_alu = Some(self.cancelled_alu_state());
        }

        let redirect_pc = self.resolve_redirect(trap, &ex);

        let control = PipelineControl {
            stalls: StallSignals {
                if_busy,
                load_use,
                ex_busy: ex.busy,
                mem_busy,
            },
            redirect_pc,
            inject_nop,
        };

        let pipeline = control.evaluate(
            current,
            StageResults {
                if_id,
                id_ex,
                ex_mem: ex.register,
                mem_wb,
            },
        );

        LegacyNextState {
            pipeline,
            alu: next_alu.expect("ALU 다음 상태는 사이클 시작 시 항상 생성되어야 합니다."),
            reset_fetch_path: redirect_pc.is_some(),
        }
    }

    fn run_mem(&mut self, bus: &mut SystemBus, trap: &mut PendingTrap) -> (bool, MemWbRegister) {
        match memory_access::execute(self, bus, self.ex_mem_reg) {
            Ok(OpStatus::Busy) => (true, MemWbRegister::default()),

            Ok(OpStatus::Complete(register)) => (false, register),

            Err(error) => {
                *trap = Some((error, self.ex_mem_reg.pc));
                (false, MemWbRegister::default())
            }
        }
    }

    fn run_ex(
        &mut self,
        mem_wb: &MemWbRegister,
        result_ready: bool,
        next_alu: &mut Option<AluNextState>,
        trap: &mut PendingTrap,
    ) -> ExOutput {
        match execute::execute(self, self.id_ex_reg, mem_wb, result_ready, next_alu) {
            Ok(OpStatus::Complete(register)) => {
                let taken = register.control.branch
                    && BranchController::get_branch_condition(
                        register.alu_result,
                        register.zero,
                        register.control.funct3,
                    );

                let branch_target = if register.control.jump || taken {
                    Some(register.target_pc)
                } else {
                    None
                };

                ExOutput {
                    register,
                    busy: false,
                    branch_target,
                }
            }

            Ok(OpStatus::Busy) => ExOutput {
                busy: true,
                ..Default::default()
            },

            Err(error) => {
                if trap.is_none() {
                    *trap = Some((error, self.id_ex_reg.pc));
                }

                ExOutput::default()
            }
        }
    }

    fn run_interrupt(&mut self, mem_wb: MemWbRegister) -> Option<LegacyNextState> {
        let cause = self.csr.check_pending_interrupt(self.privilege_mode)?;

        let return_pc = self.oldest_uncommitted_pc();

        let (target, mode) =
            self.csr
                .enter_trap(Trap { cause, tval: 0 }, return_pc, self.privilege_mode);

        self.privilege_mode = mode;

        Some(LegacyNextState {
            pipeline: PipelineState {
                pc: target,
                if_id: Default::default(),
                id_ex: Default::default(),
                ex_mem: Default::default(),

                // 인터럽트 전 완료된 MEM 결과는 보존한다.
                mem_wb,
            },
            alu: self.cancelled_alu_state(),
            reset_fetch_path: true,
        })
    }

    fn resolve_redirect(&mut self, trap: PendingTrap, ex: &ExOutput) -> Option<u32> {
        if let Some((error, epc)) = trap {
            let (target, mode) = self.csr.enter_trap(error, epc, self.privilege_mode);

            self.privilege_mode = mode;
            return Some(target);
        }

        if ex.register.control.is_mret {
            let (target, mode) = self.csr.execute_mret();
            self.privilege_mode = mode;
            return Some(target);
        }

        if ex.register.control.is_sret {
            let (target, mode) = self.csr.execute_sret();
            self.privilege_mode = mode;
            return Some(target);
        }

        ex.branch_target
    }

    fn oldest_uncommitted_pc(&self) -> u32 {
        // 기존 동작 유지.
        // valid 도입 시 PC 값 검사를 제거해야 한다.
        if self.id_ex_reg.pc != 0 {
            self.id_ex_reg.pc
        } else if self.if_id_reg.pc != 0 {
            self.if_id_reg.pc
        } else {
            self.pc
        }
    }

    fn cancelled_alu_state(&self) -> AluNextState {
        let (_, next) = self.alu.evaluate(&AluInputs {
            request: None,
            result_ready: false,
            flush: true,
        });

        next
    }

    fn commit_cycle(&mut self, bus: &mut SystemBus, next: LegacyNextState) {
        self.commit_pipeline(next.pipeline);
        self.alu.commit(next.alu);

        if next.reset_fetch_path {
            self.i_cache.reset();
            bus.reset();
        }
    }
}
