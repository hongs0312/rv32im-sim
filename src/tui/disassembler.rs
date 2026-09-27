use crate::hardware::soc::cpu::elements::decoder::Decoder;

#[rustfmt::skip]
pub fn disassemble(inst: u32) -> String {
    if inst == 0x00000013 {
        return "NOP".to_string();
    }
    if inst == 0 {
        return "ILLEGAL".to_string();
    }

    let (funct7, rs2, rs1, funct3, rd, opcode) = Decoder::decode(inst);
    let imm = Decoder::imm_gen(inst);

    match opcode {
        0x33 => { // R-Type (ALU & Multiply)
            if funct7 == 0x01 {
                let m_op = match funct3 {
                    0 => "MUL", 1 => "MULH", 2 => "MULHSU", 3 => "MULHU",
                    4 => "DIV", 5 => "DIVU", 6 => "REM", 7 => "REMU",
                    _ => "UNKNOWN_M",
                };
                format!("{} x{}, x{}, x{}", m_op, rd, rs1, rs2)
            } else {
                let op = match (funct3, funct7) {
                    (0, 0x00) => "ADD", (0, 0x20) => "SUB",
                    (1, 0x00) => "SLL", (2, 0x00) => "SLT",
                    (3, 0x00) => "SLTU", (4, 0x00) => "XOR",
                    (5, 0x00) => "SRL", (5, 0x20) => "SRA",
                    (6, 0x00) => "OR", (7, 0x00) => "AND",
                    _ => "UNKNOWN_R",
                };
                format!("{} x{}, x{}, x{}", op, rd, rs1, rs2)
            }
        }
        0x13 => { // I-Type ALU
            let op = match funct3 {
                0 => "ADDI", 2 => "SLTI", 3 => "SLTIU",
                4 => "XORI", 6 => "ORI", 7 => "ANDI",
                1 => "SLLI",
                5 => if funct7 == 0x20 { "SRAI" } else { "SRLI" },
                _ => "UNKNOWN_I",
            };
            if funct3 == 1 || funct3 == 5 {
                // 시프트 연산은 Immediate 대신 rs2 비트 위치(shamt)를 사용
                format!("{} x{}, x{}, {}", op, rd, rs1, rs2)
            } else {
                format!("{} x{}, x{}, {}", op, rd, rs1, imm as i32)
            }
        }
        0x03 => { // Load
            let op = match funct3 {
                0 => "LB", 1 => "LH", 2 => "LW", 4 => "LBU", 5 => "LHU",
                _ => "UNKNOWN_L",
            };
            format!("{} x{}, {}(x{})", op, rd, imm as i32, rs1)
        }
        0x23 => { // Store
            let op = match funct3 {
                0 => "SB", 1 => "SH", 2 => "SW",
                _ => "UNKNOWN_S",
            };
            format!("{} x{}, {}(x{})", op, rs2, imm as i32, rs1)
        }
        0x63 => { // Branch
            let op = match funct3 {
                0 => "BEQ", 1 => "BNE", 4 => "BLT", 5 => "BGE", 6 => "BLTU", 7 => "BGEU",
                _ => "UNKNOWN_B",
            };
            format!("{} x{}, x{}, {}", op, rs1, rs2, imm as i32)
        }
        0x6F => format!("JAL x{}, {}", rd, imm as i32),
        0x67 => format!("JALR x{}, x{}, {}", rd, rs1, imm as i32),
        0x37 => format!("LUI x{}, 0x{:05X}", rd, imm >> 12),
        0x17 => format!("AUIPC x{}, 0x{:05X}", rd, imm >> 12),
        _ => format!("UNKNOWN (Opcode: 0x{:02X})", opcode),
    }
}
