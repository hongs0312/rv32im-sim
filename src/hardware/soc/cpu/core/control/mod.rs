/*
    hardware/soc/cpu/core/control/mod.rs
    - 전역 제어를 위한 모듈 및 ControlSignals 구조체 정의ㄴ
*/

pub mod branch_controller;
pub mod control_unit;
pub mod forwarding_unit;
pub mod hazard_detection_unit;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlSignals {
    pub opcode: u8,
    pub funct3: u8,
    pub funct7: u8,

    // Execution 제어
    pub alu_src_a: bool, // false: rs1, true: PC (AUIPC, JAL 등에서 필요)
    pub alu_src_b: bool, // false: rs2, true: Immediate
    pub alu_op: u8,      // ALU Main Opcode

    // Branch & Jump 제어 (jal/jalr 통합)
    pub branch: bool,  // Conditional Branch (BEQ, BNE 등)
    pub jump: bool,    // Unconditional Jump (JAL, JALR 통합!)
    pub is_jalr: bool, // JALR 특화 (Base가 PC가 아닌 rs1임을 구분)

    // Memory 제어
    pub mem_read: bool,
    pub mem_write: bool,
    pub is_fence: bool,   // FENCE 명령어 처리용
    pub is_fence_i: bool, // FENCE.I 명령어 처리용

    // Write Back 제어 (3-to-1 MUX 제어로 변경)
    pub reg_write: bool,
    pub wb_src: bool, // false: ALU Output, true: Memory

    // System
    pub is_mret: bool,
    pub is_sret: bool,

    pub is_ecall: bool,
    pub is_csr: bool,
    pub csr_op: u8, // 0: Read, 1: Write, 2: Set, 3: Clear
}
impl ControlSignals {
    pub fn new(opcode: u8, funct3: u8, funct7: u8) -> Self {
        Self {
            opcode,
            funct3,
            funct7,

            ..Default::default() // 나머지 필드는 기본값으로 초기화
        }
    }
}

impl Default for ControlSignals {
    fn default() -> Self {
        Self {
            opcode: 0x13,
            funct3: 0x0,
            funct7: 0x00,

            alu_src_a: false, // rs1 기본
            alu_src_b: true,  // Imm 기본 (ADDI)
            alu_op: 0b11,     // I-Type ALU

            branch: false,
            jump: false,
            is_jalr: false,

            mem_read: false,
            mem_write: false,
            is_fence: false,   // FENCE 명령어 처리용
            is_fence_i: false, // FENCE.I 명령어 처리용

            reg_write: false, // NOP일 때 Safe
            wb_src: false,    // ALU 결과 기본

            is_mret: false,
            is_sret: false,

            is_ecall: false,
            is_csr: false,
            csr_op: 0,
        }
    }
}
