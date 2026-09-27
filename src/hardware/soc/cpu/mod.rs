/*
    CPU 모듈
    - CPU는 5단계 파이프라인 구조를 가지며, 각 단계는 별도의 레지스터를 사용하여 데이터를 전달함
*/

pub mod elements;
pub mod pipeline_stage;

use crate::hardware::soc::system_bus::SystemBus;
use elements::{
    alu::Alu, cache::L1Cache, control::branch_controller::BranchController,
    hazard_detection_unit::HazardDetectionUnit, register::RegisterFile,
};

use pipeline_stage::{ExMemRegister, IdExRegister, IfIdRegister, MemWbRegister, StageStatus};
use pipeline_stage::{execute, instruction_decode, instruction_fetch, memory_access, write_back};

pub struct Cpu {
    pub pc: u32,
    pub regs: RegisterFile,
    pub alu: Alu,

    pub i_cache: L1Cache, // L1 명령어 캐시
    pub d_cache: L1Cache, // L1 데이터 캐시

    pub if_id_reg: IfIdRegister,
    pub id_ex_reg: IdExRegister,
    pub ex_mem_reg: ExMemRegister,
    pub mem_wb_reg: MemWbRegister,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            pc: 0,
            regs: RegisterFile::new(),
            alu: Alu::new(),

            i_cache: L1Cache::new(),
            d_cache: L1Cache::new(),

            if_id_reg: IfIdRegister::default(),
            id_ex_reg: IdExRegister::default(),
            ex_mem_reg: ExMemRegister::default(),
            mem_wb_reg: MemWbRegister::default(),
        }
    }

    pub fn pipeline_step(&mut self, mem_bus: &mut SystemBus, inject_nop: bool) {
        // 1. WB 및 MEM 단계 실행 (역순)
        self.write_back(self.mem_wb_reg);

        let mem_status = self.memory_access(mem_bus, self.ex_mem_reg);
        let is_mem_busy = matches!(mem_status, StageStatus::Busy);

        let next_mem_wb_reg = match mem_status {
            StageStatus::Complete(reg) => reg,
            StageStatus::Busy => MemWbRegister::default(),
        };

        // 3. EX 단계 실행
        let ex_status = match is_mem_busy {
            true => StageStatus::Busy,
            false => self.execute(self.id_ex_reg, &next_mem_wb_reg),
        };
        let is_ex_busy = matches!(ex_status, StageStatus::Busy);

        // EX 단계 래치 결과 및 분기 판정 제어 신호 추출
        let (next_ex_mem_reg, pcsrc, branch_target) = match ex_status {
            StageStatus::Complete(ex_reg) => {
                let branch_taken = ex_reg.control.branch
                    && BranchController::get_branch_condition(
                        ex_reg.alu_result,
                        ex_reg.zero,
                        ex_reg.control.funct3,
                    );

                let is_pcsrc = ex_reg.control.jump || branch_taken;

                (ex_reg, is_pcsrc, ex_reg.target_pc)
            }
            StageStatus::Busy => (ExMemRegister::default(), false, 0),
        };

        // 4. 앞단 실행 (ID, IF)
        let is_stall = HazardDetectionUnit::check_load_use(
            self.id_ex_reg.control.mem_read,
            self.id_ex_reg.rd,
            self.if_id_reg.instruction,
        );

        let next_id_ex_reg = self.instruction_decode(self.if_id_reg);

        let if_status = self.instruction_fetch(mem_bus, inject_nop);
        let is_if_busy = matches!(if_status, StageStatus::Busy);

        let next_if_id_reg = match if_status {
            StageStatus::Complete(reg) => reg,
            StageStatus::Busy => IfIdRegister::default(), // 스톨 시 NOP처럼 동작
        };

        let stall_mem = is_mem_busy;
        let stall_ex = is_mem_busy || is_ex_busy;
        let stall_id = stall_ex || is_stall;
        let stall_if = stall_id || is_if_busy;

        // Ghost Stall 무시: 어차피 점프(pcsrc)로 인해 버려질 명령어들이 만든 스톨은 무시합니다!
        if pcsrc {
            // 분기/점프 발생 시 스톨 무시
            self.pc = branch_target;
            self.if_id_reg = IfIdRegister::default();
            self.id_ex_reg = IdExRegister::default();

            self.i_cache.reset();
            mem_bus.reset();
        } else {
            // 5. 래치 업데이트
            if !stall_if && !inject_nop {
                self.pc = self.pc.wrapping_add(4);
            }

            if !stall_id {
                self.if_id_reg = match is_if_busy {
                    true => IfIdRegister::default(), // 스톨 시 NOP처럼 동작
                    false => next_if_id_reg,
                };
            }

            if !stall_ex {
                self.id_ex_reg = match is_stall {
                    true => IdExRegister::default(), // 스톨 시 NOP처럼 동작
                    false => next_id_ex_reg,
                };
            }
        }

        if !stall_mem {
            self.ex_mem_reg = match is_ex_busy {
                true => ExMemRegister::default(),
                false => next_ex_mem_reg,
            }
        }

        self.mem_wb_reg = match is_mem_busy {
            true => MemWbRegister::default(),
            false => next_mem_wb_reg,
        };
    }

    // 파이프라인 단계별 실행 메서드
    // 각 단계의 실행은 pipeline_stage 모듈의 execute 함수를 호출하여 수행
    fn instruction_fetch(
        &mut self,
        mem_bus: &mut SystemBus,
        inject_nop: bool,
    ) -> StageStatus<IfIdRegister> {
        instruction_fetch::execute(self, mem_bus, inject_nop)
    }

    fn instruction_decode(&mut self, if_id_reg: IfIdRegister) -> IdExRegister {
        instruction_decode::execute(self, if_id_reg)
    }

    fn execute(
        &mut self,
        id_ex_reg: IdExRegister,
        next_mem_wb_reg: &MemWbRegister,
    ) -> StageStatus<ExMemRegister> {
        execute::execute(self, id_ex_reg, next_mem_wb_reg)
    }

    fn memory_access(
        &mut self,
        mem_bus: &mut SystemBus,
        ex_mem_reg: ExMemRegister,
    ) -> StageStatus<MemWbRegister> {
        memory_access::execute(self, mem_bus, ex_mem_reg)
    }

    fn write_back(&mut self, mem_wb_reg: MemWbRegister) {
        write_back::execute(self, mem_wb_reg)
    }
}
