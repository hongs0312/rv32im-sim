pub struct Decoder;

impl Decoder {
    pub fn new() -> Self {
        Decoder
    }

    pub fn decode(inst: u32) -> (u8, u8, u8, u8, u8, u8) {
        // Decode the opcode
        // 0000 0000 0000 0000 0000 0000 0000 0000
        // 31 <- 0순으로 인덱싱

        let opcode = (inst & 0x7f) as u8; // 0~6 bits
        let rd = ((inst >> 7) & 0x1f) as u8; // 7~11 bits
        let funct3 = ((inst >> 12) & 0x7) as u8; // 12~14 bits
        let rs1 = ((inst >> 15) & 0x1f) as u8; // 15~19 bits
        let rs2 = ((inst >> 20) & 0x1f) as u8; // 20~24 bits
        let funct7 = ((inst >> 25) & 0x7f) as u8; // 25~31 bits

        // core instruction format에 따라 변환
        (funct7, rs2, rs1, funct3, rd, opcode)
    }

    pub fn imm_gen(inst: u32) -> i32 {
        let opcode = inst & 0x7f; // 0~6 bits
        match opcode {
            0x03 | 0x13 | 0x67 => {
                // I-Type
                let imm = (inst as i32) >> 20; // Sign-extend the immediate

                imm
            }
            0x23 => {
                // S-Type
                let imm_11_5 = (inst >> 25) & 0x7f; // bits 25~31
                let imm_4_0 = (inst >> 7) & 0x1f; // bits 7~11
                let imm = ((imm_11_5 << 5) | imm_4_0) as i32;
                (imm << 20) >> 20 // Sign-extend to 32 bits
            }
            0x63 => {
                // B-Type
                let imm_12 = (inst >> 31) & 0x1; // bit 31
                let imm_10_5 = (inst >> 25) & 0x3f; // bits 25~30
                let imm_4_1 = (inst >> 8) & 0xf; // bits 8~11
                let imm_11 = (inst >> 7) & 0x1; // bit 7
                let imm =
                    ((imm_12 << 12) | (imm_11 << 11) | (imm_10_5 << 5) | (imm_4_1 << 1)) as i32;
                (imm << 19) >> 19 // Sign-extend to 32 bits
            }
            0x37 | 0x17 => {
                // U-Type
                let imm = (inst & 0xfffff000) as i32; // 상위 20비트만 사용, 하위 12비트는 0으로 채움
                imm
            }
            0x6f => {
                // J-Type
                let imm_20 = (inst >> 31) & 0x1; // bit 31
                let imm_10_1 = (inst >> 21) & 0x3ff; // bits 21~30
                let imm_11 = (inst >> 20) & 0x1; // bit 20
                let imm_19_12 = (inst >> 12) & 0xff; // bits 12~19

                let raw_imm = (imm_20 << 20) | (imm_19_12 << 12) | (imm_11 << 11) | (imm_10_1 << 1);
                // Sign Extension (bit 20 기준)
                let imm = if (raw_imm & 0x0010_0000) != 0 {
                    raw_imm | 0xFFE0_0000
                } else {
                    raw_imm
                };

                imm as i32
            }

            _ => 0, // 기본값: 0 (알 수 없는 명령어에 대해)
        }
    }
}

#[test]
fn decoder_test() {
    let inst = 0b0000000_00001_00000_000_00000_0010011; // ADDI x0, x0, 1
    let (funct7, rs2, rs1, funct3, rd, opcode) = Decoder::decode(inst);
    assert_eq!(funct7, 0);
    assert_eq!(rs2, 1);
    assert_eq!(rs1, 0);
    assert_eq!(funct3, 0);
    assert_eq!(rd, 0);
    assert_eq!(opcode, 0b0010011);
}
