/*
    CPU & Core 모듈
    - Core는 독립적인 명령어 실행 단위(RISC-V Hart)로, 5단계 파이프라인과 전용 L1 캐시를 포함함
    - 향후 멀티코어(Multi-Hart), 분기 예측기(Branch Predictor), OoO 확장을 고려한 모듈화 구조
*/

pub mod components;
pub mod control;
pub mod pipeline_stage;

use crate::hardware::soc::cpu::core::components::csr::CsrFile;
use crate::hardware::soc::system_bus::SystemBus;
use crate::hardware::soc::types::OpStatus;

use components::{alu::Alu, cache::L1Cache, register::RegisterFile};
use control::{branch_controller::BranchController, hazard_detection_unit::HazardDetectionUnit};

use pipeline_stage::{ExMemRegister, IdExRegister, IfIdRegister, MemWbRegister};
use pipeline_stage::{execute, instruction_decode, instruction_fetch, memory_access, write_back};

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PrivilegeMode {
    User = 0b00,       // 일반 애플리케이션 모드 (U-Mode)
    Supervisor = 0b01, // OS 커널 모드 (S-Mode)
    Machine = 0b11,    // 최상위 하드웨어/펌웨어 모드 (M-Mode)
}

impl PrivilegeMode {
    pub fn from_bits(bits: u32) -> Self {
        match bits & 0b11 {
            0b00 => Self::User,
            0b01 => Self::Supervisor,
            _ => Self::Machine,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trap {
    pub cause: u32, // scause 또는 mcause에 들어갈 예외 번호
    pub tval: u32,  // stval 또는 mtval에 들어갈 문제의 가상 주소/명령어
}

/// EX 단계의 실행 결과와 분기 판정을 묶어서 전달하는 내부 구조체
struct ExStageOutput {
    next_reg: ExMemRegister,
    is_busy: bool,
    pcsrc: bool,
    branch_target: u32,
}

/// 각 파이프라인 단계의 스톨(Stall) 및 비지(Busy) 신호 묶음
struct PipelineStalls {
    if_busy: bool,
    load_use_hazard: bool,
    ex_busy: bool,
    mem_busy: bool,
}

/// 단일 프로세서 코어 (RISC-V Hart)
/// - 파이프라인 레지스터, 전용 L1 캐시, 연산기, 해저드 제어기를 하나로 캡슐화
pub struct Core {
    pub hart_id: usize, // 멀티코어 확장을 위한 코어 식별자 (mhartid)
    pub pc: u32,
    pub regs: RegisterFile,
    pub alu: Alu,
    pub privilege_mode: PrivilegeMode,

    pub csr: CsrFile,

    // 코어 전용(Private) L1 캐시
    pub i_cache: L1Cache,
    pub d_cache: L1Cache,

    // 코어 전용 파이프라인 래치
    pub if_id_reg: IfIdRegister,
    pub id_ex_reg: IdExRegister,
    pub ex_mem_reg: ExMemRegister,
    pub mem_wb_reg: MemWbRegister,
    // TODO: 향후 확장 시 이곳에 BranchPredictor, ReorderBuffer(OoO) 등을 코어 단위로 장착
}

// 기존 코드(pipeline_stage, SoC 등)와의 하위 호환성을 위한 타입 별칭
// 추후 멀티코어 전환 시 `pub struct Cpu { pub cores: Vec<Core> }` 형태로 분리 가능
pub type Cpu = Core;

impl Core {
    pub fn new() -> Self {
        Self::new_with_id(0)
    }

    pub fn new_with_id(hart_id: usize) -> Self {
        Self {
            hart_id,
            pc: 0,
            regs: RegisterFile::new(),
            alu: Alu::new(),
            csr: CsrFile::new(),
            privilege_mode: PrivilegeMode::Machine,

            i_cache: L1Cache::new(true),
            d_cache: L1Cache::new(false),

            if_id_reg: IfIdRegister::default(),
            id_ex_reg: IdExRegister::default(),
            ex_mem_reg: ExMemRegister::default(),
            mem_wb_reg: MemWbRegister::default(),
        }
    }

    /// 1클럭 사이클 동안의 5단계 파이프라인 실행 (역순 진행: WB -> MEM -> EX -> ID -> IF)
    pub fn pipeline_step(&mut self, mem_bus: &mut SystemBus, inject_nop: bool) {
        let mut pending_trap: Option<(Trap, u32)> = None; // (Trap, 발생 시점의 EPC)

        // 1. WB 및 MEM 단계 실행
        self.write_back(self.mem_wb_reg);
        let (is_mem_busy, next_mem_wb_reg) = self.step_mem_stage(mem_bus, &mut pending_trap);

        // 2. 비동기 하드웨어 인터럽트 체크 (MEM 단계가 바쁘지 않을 때만 진입)
        if !is_mem_busy && self.check_and_handle_interrupt(mem_bus, next_mem_wb_reg) {
            return;
        }

        // 3. EX 단계 실행 (ALU 연산 및 분기 조건 판정)
        let ex_out = self.step_ex_stage(&next_mem_wb_reg, &mut pending_trap);

        // 4. ID 및 IF 앞단 실행 (해저드 감지 포함)
        let is_load_use_stall = self.check_load_use_hazard();
        let next_id_ex_reg = self.instruction_decode(self.if_id_reg);
        let (is_if_busy, next_if_id_reg) =
            self.step_if_stage(mem_bus, inject_nop, &mut pending_trap);

        // 5. 제어 흐름 변경(Trap, mret/sret, 분기/점프) 타겟 PC 결정
        let redirect_pc = self.resolve_pc_redirect(pending_trap, &ex_out);

        // 6. 파이프라인 래치 및 PC 업데이트 (플러시 또는 스톨 반영)
        let stalls = PipelineStalls {
            if_busy: is_if_busy,
            load_use_hazard: is_load_use_stall,
            ex_busy: ex_out.is_busy,
            mem_busy: is_mem_busy,
        };

        self.update_latches(
            mem_bus,
            redirect_pc,
            inject_nop,
            &stalls,
            next_if_id_reg,
            next_id_ex_reg,
            ex_out.next_reg,
            next_mem_wb_reg,
        );
    }

    // ========================================================================
    // 파이프라인 스테이지 제어 및 판정 래퍼 메서드
    // ========================================================================

    /// MEM 단계 실행 및 예외 포착 래퍼
    fn step_mem_stage(
        &mut self,
        mem_bus: &mut SystemBus,
        pending_trap: &mut Option<(Trap, u32)>,
    ) -> (bool, MemWbRegister) {
        match self.memory_access(mem_bus, self.ex_mem_reg) {
            Ok(OpStatus::Busy) => (true, MemWbRegister::default()),
            Ok(OpStatus::Complete(reg)) => (false, reg),
            Err(trap) => {
                // 가장 오래된 명령어(MEM 단계)의 예외이므로 최우선 등록
                *pending_trap = Some((trap, self.ex_mem_reg.pc));
                (false, MemWbRegister::default())
            }
        }
    }

    /// EX 단계 실행, 분기 조건 평가 및 예외 포착 래퍼
    fn step_ex_stage(
        &mut self,
        next_mem_wb_reg: &MemWbRegister,
        pending_trap: &mut Option<(Trap, u32)>,
    ) -> ExStageOutput {
        match self.execute(self.id_ex_reg, next_mem_wb_reg) {
            Ok(OpStatus::Complete(ex_reg)) => {
                let branch_taken = ex_reg.control.branch
                    && BranchController::get_branch_condition(
                        ex_reg.alu_result,
                        ex_reg.zero,
                        ex_reg.control.funct3,
                    );
                let pcsrc = ex_reg.control.jump || branch_taken;

                ExStageOutput {
                    next_reg: ex_reg,
                    is_busy: false,
                    pcsrc,
                    branch_target: ex_reg.target_pc,
                }
            }
            Ok(OpStatus::Busy) => ExStageOutput {
                next_reg: ExMemRegister::default(),
                is_busy: true,
                pcsrc: false,
                branch_target: 0,
            },
            Err(trap) => {
                // 앞선 MEM 단계에서 예외가 없었을 때만 EX 예외 등록
                if pending_trap.is_none() {
                    *pending_trap = Some((trap, self.id_ex_reg.pc));
                }
                ExStageOutput {
                    next_reg: ExMemRegister::default(),
                    is_busy: false,
                    pcsrc: false,
                    branch_target: 0,
                }
            }
        }
    }

    /// IF 단계 실행 및 예외 포착 래퍼
    fn step_if_stage(
        &mut self,
        mem_bus: &mut SystemBus,
        inject_nop: bool,
        pending_trap: &mut Option<(Trap, u32)>,
    ) -> (bool, IfIdRegister) {
        match self.instruction_fetch(mem_bus, inject_nop) {
            Ok(OpStatus::Complete(reg)) => (false, reg),
            Ok(OpStatus::Busy) => (true, IfIdRegister::default()),
            Err(trap) => {
                if pending_trap.is_none() {
                    *pending_trap = Some((trap, self.pc));
                }
                (false, IfIdRegister::default())
            }
        }
    }

    // ========================================================================
    // 트랩, 분기 리다이렉트 및 래치 업데이트 헬퍼
    // ========================================================================

    /// 하드웨어 비동기 인터럽트 확인 및 즉시 트랩 진입 처리
    fn check_and_handle_interrupt(
        &mut self,
        mem_bus: &mut SystemBus,
        next_mem_wb_reg: MemWbRegister,
    ) -> bool {
        if let Some(cause) = self.csr.check_pending_interrupt(self.privilege_mode) {
            let return_pc = self.oldest_uncommitted_pc();
            let trap = Trap { cause, tval: 0 };
            let (trap_target_pc, new_mode) =
                self.csr.enter_trap(trap, return_pc, self.privilege_mode);

            self.privilege_mode = new_mode;

            self.redirect_and_flush(trap_target_pc, mem_bus);
            self.ex_mem_reg = ExMemRegister::default();
            self.mem_wb_reg = next_mem_wb_reg;
            return true;
        }
        false
    }

    /// 동기 예외(Trap), 특권 복귀(mret/sret), 분기/점프(pcsrc)에 따른 다음 타겟 PC 결정
    fn resolve_pc_redirect(
        &mut self,
        pending_trap: Option<(Trap, u32)>,
        ex_out: &ExStageOutput,
    ) -> Option<u32> {
        if let Some((trap, epc)) = pending_trap {
            let (target_pc, new_mode) = self.csr.enter_trap(trap, epc, self.privilege_mode);
            self.privilege_mode = new_mode;
            Some(target_pc)
        } else if ex_out.next_reg.control.is_mret {
            let (target_pc, new_mode) = self.csr.execute_mret();
            self.privilege_mode = new_mode;
            Some(target_pc)
        } else if ex_out.next_reg.control.is_sret {
            let (target_pc, new_mode) = self.csr.execute_sret();
            self.privilege_mode = new_mode;
            Some(target_pc)
        } else if ex_out.pcsrc {
            Some(ex_out.branch_target)
        } else {
            None
        }
    }

    /// 스톨 신호 및 리다이렉트 여부에 따라 파이프라인 레지스터(래치)를 일괄 갱신
    #[allow(clippy::too_many_arguments)]
    fn update_latches(
        &mut self,
        mem_bus: &mut SystemBus,
        redirect_pc: Option<u32>,
        inject_nop: bool,
        stalls: &PipelineStalls,
        next_if_id: IfIdRegister,
        next_id_ex: IdExRegister,
        next_ex_mem: ExMemRegister,
        next_mem_wb: MemWbRegister,
    ) {
        let stall_mem = stalls.mem_busy;
        let stall_ex = stall_mem || stalls.ex_busy;
        let stall_id = stall_ex || stalls.load_use_hazard;
        let stall_if = stall_id || stalls.if_busy;

        if let Some(target_pc) = redirect_pc {
            // 분기/점프/예외 발생 시 앞단(IF, ID) 플러시 및 PC 강제 이동
            self.redirect_and_flush(target_pc, mem_bus);
        } else {
            if !stall_if && !inject_nop {
                self.pc = self.pc.wrapping_add(4);
            }
            if !stall_id {
                self.if_id_reg = if stalls.if_busy {
                    IfIdRegister::default()
                } else {
                    next_if_id
                };
            }
            if !stall_ex {
                self.id_ex_reg = if stalls.load_use_hazard {
                    IdExRegister::default()
                } else {
                    next_id_ex
                };
            }
        }

        if !stall_mem {
            self.ex_mem_reg = if stalls.ex_busy {
                ExMemRegister::default()
            } else {
                next_ex_mem
            };
        }

        self.mem_wb_reg = if stalls.mem_busy {
            MemWbRegister::default()
        } else {
            next_mem_wb
        };
    }

    fn check_load_use_hazard(&self) -> bool {
        HazardDetectionUnit::check_load_use(
            self.id_ex_reg.control.mem_read,
            self.id_ex_reg.rd,
            self.if_id_reg.instruction,
        )
    }

    fn oldest_uncommitted_pc(&self) -> u32 {
        if self.id_ex_reg.pc != 0 {
            self.id_ex_reg.pc
        } else if self.if_id_reg.pc != 0 {
            self.if_id_reg.pc
        } else {
            self.pc
        }
    }

    fn redirect_and_flush(&mut self, target_pc: u32, mem_bus: &mut SystemBus) {
        self.pc = target_pc;
        self.if_id_reg = IfIdRegister::default();
        self.id_ex_reg = IdExRegister::default();
        self.i_cache.reset();
        mem_bus.reset();
    }

    // ========================================================================
    // 하위 파이프라인 스테이지 모듈 호출부
    // ========================================================================

    fn instruction_fetch(
        &mut self,
        mem_bus: &mut SystemBus,
        inject_nop: bool,
    ) -> Result<OpStatus<IfIdRegister>, Trap> {
        instruction_fetch::execute(self, mem_bus, inject_nop)
    }

    fn instruction_decode(&mut self, if_id_reg: IfIdRegister) -> IdExRegister {
        instruction_decode::execute(self, if_id_reg)
    }

    fn execute(
        &mut self,
        id_ex_reg: IdExRegister,
        next_mem_wb_reg: &MemWbRegister,
    ) -> Result<OpStatus<ExMemRegister>, Trap> {
        execute::execute(self, id_ex_reg, next_mem_wb_reg)
    }

    fn memory_access(
        &mut self,
        mem_bus: &mut SystemBus,
        ex_mem_reg: ExMemRegister,
    ) -> Result<OpStatus<MemWbRegister>, Trap> {
        memory_access::execute(self, mem_bus, ex_mem_reg)
    }

    fn write_back(&mut self, mem_wb_reg: MemWbRegister) {
        write_back::execute(self, mem_wb_reg)
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}
