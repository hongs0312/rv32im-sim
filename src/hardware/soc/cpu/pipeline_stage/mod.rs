pub mod execute;
pub mod instruction_decode;
pub mod instruction_fetch;
pub mod memory_access;
pub mod write_back;

use super::elements::control::ControlSignals;

// pipline register structures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IfIdRegister {
    pub pc: u32,
    pub instruction: u32,
}
impl Default for IfIdRegister {
    fn default() -> Self {
        Self {
            pc: 0,
            instruction: 0x00000013, // NOP 명령어로 초기화
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ExMemRegister {
    pub control: ControlSignals,

    pub target_pc: u32,

    pub zero: bool,
    pub alu_result: u32,

    pub rd: u8,
    pub rs2_data: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct MemWbRegister {
    pub control: ControlSignals,

    pub alu_result: u32,
    pub mem_data: u32,
    pub rd: u8,
}
