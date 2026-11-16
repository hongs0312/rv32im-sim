use super::components::alu::AluNextState;
use super::control::ControlSignals;

/// IF → ID 파이프라인 레지스터.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IfIdRegister {
    pub pc: u32,
    pub instruction: u32,
}

impl Default for IfIdRegister {
    fn default() -> Self {
        Self {
            pc: 0,
            instruction: 0x0000_0013,
        }
    }
}

/// ID → EX 파이프라인 레지스터.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdExRegister {
    pub control: ControlSignals,

    pub pc: u32,

    pub rd: u8,
    pub rs1: u8,
    pub rs2: u8,

    pub rs1_data: u32,
    pub rs2_data: u32,

    pub imm: i32,
}

/// EX → MEM 파이프라인 레지스터.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExMemRegister {
    pub control: ControlSignals,

    pub pc: u32,
    pub target_pc: u32,

    pub zero: bool,
    pub alu_result: u32,

    pub rd: u8,
    pub rs2_data: u32,
}

/// MEM → WB 파이프라인 레지스터.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemWbRegister {
    pub control: ControlSignals,

    pub alu_result: u32,
    pub mem_data: u32,
    pub rd: u8,
}

/// PC와 파이프라인 레지스터 값의 묶음.
///
/// 현재 Core 필드에서 읽은 스냅샷 또는 다음 상태로 사용한다.
/// 별도의 영구 상태로 중복 저장하지 않는다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PipelineState {
    pub pc: u32,

    pub if_id: IfIdRegister,
    pub id_ex: IdExRegister,
    pub ex_mem: ExMemRegister,
    pub mem_wb: MemWbRegister,
}

/// 한 사이클의 단계별 계산 결과.
///
/// 아직 파이프라인 레지스터에 반영되지 않은 입력 후보다.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct StageResults {
    pub if_id: IfIdRegister,
    pub id_ex: IdExRegister,
    pub ex_mem: ExMemRegister,
    pub mem_wb: MemWbRegister,
}

/// 이번 전환 단계에서 마지막에 반영할 상태.
///
/// CSR·캐시·버스 상태까지 포함하는 완전한 Core 다음 상태는 아니다.
pub(crate) struct LegacyNextState {
    pub pipeline: PipelineState,
    pub alu: AluNextState,

    /// 기존 리다이렉트 시 호출하던 reset 처리.
    pub reset_fetch_path: bool,
}
