pub mod branch_controller;
pub mod control_unit;

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

    // Write Back 제어 (3-to-1 MUX 제어로 변경)
    pub reg_write: bool,
    pub wb_src: bool, // false: ALU Output, true: Memory

    // System
    pub is_ecall: bool,
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

            reg_write: false, // NOP일 때 Safe
            wb_src: false,    // ALU 결과 기본

            is_ecall: false,
        }
    }
}
