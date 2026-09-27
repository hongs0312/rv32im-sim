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
        0x73 => { // SYSTEM (ECALL, EBREAK, CSR)
            let imm_12 = imm & 0xFFF; // I-Type 형태의 상위 12비트
            match funct3 {
                0 => {
                    match imm_12 {
                        0 => "ECALL".to_string(),
                        1 => "EBREAK".to_string(),
                        0x302 => "MRET".to_string(), // 특권 모드 복귀 명령어 (필요시)
                        _ => format!("SYSTEM_PRIV (0x{:03X})", imm_12),
                    }
                }
                _ => "UNKNOWN_SYSTEM".to_string(),
            }
        }
        0x6F => format!("JAL x{}, {}", rd, imm as i32),
        0x67 => format!("JALR x{}, x{}, {}", rd, rs1, imm as i32),
        0x37 => format!("LUI x{}, 0x{:05X}", rd, imm >> 12),
        0x17 => format!("AUIPC x{}, 0x{:05X}", rd, imm >> 12),
        _ => format!("UNKNOWN (Opcode: 0x{:02X})", opcode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disassemble_special() {
        assert_eq!(disassemble(0x00000013), "NOP");
        assert_eq!(disassemble(0x00000000), "ILLEGAL");
    }

    #[test]
    fn test_disassemble_r_type() {
        // ADD x1, x2, x3 (opcode: 0x33, funct3: 0, funct7: 0, rd: 1, rs1: 2, rs2: 3)
        assert_eq!(disassemble(0x003100B3), "ADD x1, x2, x3");
    }

    #[test]
    fn test_disassemble_m_type() {
        // MUL x4, x5, x6 (opcode: 0x33, funct3: 0, funct7: 1, rd: 4, rs1: 5, rs2: 6)
        assert_eq!(disassemble(0x02628233), "MUL x4, x5, x6");
    }

    #[test]
    fn test_disassemble_i_type() {
        // ADDI x1, x2, -16 (opcode: 0x13, funct3: 0, rd: 1, rs1: 2, imm: -16)
        assert_eq!(disassemble(0xFF010093), "ADDI x1, x2, -16");

        // SLLI x1, x2, 4 (opcode: 0x13, funct3: 1, rd: 1, rs1: 2, rs2/shamt: 4)
        assert_eq!(disassemble(0x00411093), "SLLI x1, x2, 4");
    }

    #[test]
    fn test_disassemble_load() {
        // LW x7, 8(x8) (opcode: 0x03, funct3: 2, rd: 7, rs1: 8, imm: 8)
        assert_eq!(disassemble(0x00842383), "LW x7, 8(x8)");
    }

    #[test]
    fn test_disassemble_store() {
        // SW x9, 16(x10) (opcode: 0x23, funct3: 2, rs1: 10, rs2: 9, imm: 16)
        assert_eq!(disassemble(0x00952823), "SW x9, 16(x10)");
    }

    #[test]
    fn test_disassemble_branch() {
        // BEQ x0, x0, -4 (opcode: 0x63, funct3: 0, rs1: 0, rs2: 0, imm: -4)
        // 무한 루프 도는 분기문 패턴
        assert_eq!(disassemble(0xFE000EE3), "BEQ x0, x0, -4");
    }

    #[test]
    fn test_disassemble_u_type() {
        // LUI x5, 0x12345 (opcode: 0x37, rd: 5, imm: 0x12345000)
        assert_eq!(disassemble(0x123452B7), "LUI x5, 0x12345");
    }

    #[test]
    fn test_disassemble_j_type() {
        // JAL x1, -4 (opcode: 0x6F, rd: 1, imm: -4)
        assert_eq!(disassemble(0xFFDFF0EF), "JAL x1, -4");
    }
}
