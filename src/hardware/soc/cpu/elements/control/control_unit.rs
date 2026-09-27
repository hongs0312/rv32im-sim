use super::ControlSignals;

pub struct ControlUnit;

impl ControlUnit {
    pub fn decode(opcode: u8, funct3: u8, funct7: u8) -> ControlSignals {
        let mut control = ControlSignals::default();

        control.opcode = opcode;
        control.funct3 = funct3;
        control.funct7 = funct7;

        match opcode {
            0x37 => {
                // LUI
                control.alu_src_b = true;
                control.alu_op = 0b100; // LUI Pass-through
                control.reg_write = true;
                control.wb_src = false;
            }
            0x17 => {
                // AUIPC
                control.alu_src_a = true; // PC 선택
                control.alu_src_b = true; // Imm 선택
                control.alu_op = 0b00; // ADD 수행 (PC + Imm)
                control.reg_write = true;
                control.wb_src = false;
            }
            0x33 => {
                // R-Type
                control.alu_src_a = false;
                control.alu_src_b = false;
                control.alu_op = 0b10;
                control.reg_write = true;
                control.wb_src = false;
            }
            0x13 => {
                // I-Type ALU
                control.alu_src_a = false;
                control.alu_src_b = true;
                control.alu_op = 0b11;
                control.reg_write = true;
                control.wb_src = false;
            }
            0x03 => {
                // Load
                control.alu_src_a = false;
                control.alu_src_b = true;
                control.alu_op = 0b00; // ADD (Addr = rs1 + Imm)
                control.mem_read = true;
                control.reg_write = true;
                control.wb_src = true; // Memory 읽기 값 선택
            }
            0x23 => {
                // Store
                control.alu_src_a = false;
                control.alu_src_b = true;
                control.alu_op = 0b00; // ADD (Addr = rs1 + Imm)
                control.mem_write = true;
            }
            0x63 => {
                // Branch
                control.alu_src_a = false;
                control.alu_src_b = false;
                control.alu_op = 0b01; // Branch Compare
                control.branch = true;
            }
            0x6F => {
                // JAL
                control.alu_src_a = true; // PC 선택
                control.alu_src_b = true; // Imm 선택 (Target = PC + Imm)
                control.alu_op = 0b00; // ADD
                control.jump = true; // JAL/JALR 공통 Jump 신호
                control.reg_write = true;
                control.wb_src = false; // PC + 4 선택!
            }
            0x67 => {
                // JALR
                control.alu_src_a = false; // rs1 선택! (Target = rs1 + Imm)
                control.alu_src_b = true; // Imm 선택
                control.alu_op = 0b00; // ADD
                control.jump = true; // JAL/JALR 공통 Jump 신호
                control.is_jalr = true; // LSB Masking(0bit Clear) 처리를 위해 표시
                control.reg_write = true;
                control.wb_src = false; // PC + 4 선택!
            }
            0x73 => {
                // System
                control.is_ecall = true;
            }
            _ => {}
        }

        control
    }
}
