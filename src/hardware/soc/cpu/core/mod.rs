// /*
//     CPU & Core 모듈
//     - Core는 독립적인 명령어 실행 단위(RISC-V Hart)로, 5단계 파이프라인과 전용 L1 캐시를 포함함
//     - 향후 멀티코어(Multi-Hart), 분기 예측기(Branch Predictor), OoO 확장을 고려한 모듈화 구조
// */
pub mod clocked_impl;
pub mod components;
pub mod control;
pub mod pipeline_registers;
pub mod pipeline_stage;

#[cfg(test)]
mod tests;

mod datapath;

use components::{alu::Alu, cache::L1Cache, csr::CsrFile, register::RegisterFile};

use pipeline_registers::{ExMemRegister, IdExRegister, IfIdRegister, MemWbRegister, PipelineState};

pub use clocked_impl::{
    CoreInputs, CoreNextState, CoreOutputs, CoreRedirect, CoreStageSignals, CoreStallSignals,
    RedirectScope, RegisterWrite,
};

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PrivilegeMode {
    User = 0b00,
    Supervisor = 0b01,
    Machine = 0b11,
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
    pub cause: u32,
    pub tval: u32,
}

/// 5단계 데이터패스를 소유하고 연결하는 단일 Hart.
pub struct Core {
    pub hart_id: usize,

    pub regs: RegisterFile,
    pub csr: CsrFile,
    pub privilege_mode: PrivilegeMode,

    pub alu: Alu,
    pub i_cache: L1Cache,
    pub d_cache: L1Cache,

    // 현재 순차 상태.
    // 기존 외부 코드와의 호환성을 위해 필드명을 유지한다.
    pub pc: u32,
    pub if_id_reg: IfIdRegister,
    pub id_ex_reg: IdExRegister,
    pub ex_mem_reg: ExMemRegister,
    pub mem_wb_reg: MemWbRegister,
}

/// 기존 경로를 사용하는 코드의 호환성을 위한 별칭.
pub type Cpu = Core;

impl Core {
    pub fn new() -> Self {
        Self::new_with_id(0)
    }

    pub fn new_with_id(hart_id: usize) -> Self {
        Self {
            hart_id,

            regs: RegisterFile::new(),
            csr: CsrFile::new(),
            privilege_mode: PrivilegeMode::Machine,

            alu: Alu::new(),

            i_cache: L1Cache::new(true),
            d_cache: L1Cache::new(false),

            pc: 0,
            if_id_reg: IfIdRegister::default(),
            id_ex_reg: IdExRegister::default(),
            ex_mem_reg: ExMemRegister::default(),
            mem_wb_reg: MemWbRegister::default(),
        }
    }

    /// 현재 파이프라인 레지스터의 읽기 전용 스냅샷.
    pub fn pipeline_state(&self) -> PipelineState {
        PipelineState {
            pc: self.pc,
            if_id: self.if_id_reg,
            id_ex: self.id_ex_reg,
            ex_mem: self.ex_mem_reg,
            mem_wb: self.mem_wb_reg,
        }
    }

    /// 계산이 끝난 파이프라인 상태를 반영한다.
    fn commit_pipeline(&mut self, next: PipelineState) {
        self.pc = next.pc;
        self.if_id_reg = next.if_id;
        self.id_ex_reg = next.id_ex;
        self.ex_mem_reg = next.ex_mem;
        self.mem_wb_reg = next.mem_wb;
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}
