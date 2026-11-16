//! 기존 ALU 제어 신호를 명시적인 연산 종류로 변환한다.
//!
//! 상태를 변경하지 않는 조합 디코드 로직이다.

use super::types::{AluDecodeError, AluOperation};

impl AluOperation {
    pub fn decode(alu_op: u8, funct3: u8, funct7: u8) -> Result<Self, AluDecodeError> {
        let operation = match alu_op {
            // 주소 계산, AUIPC, JAL/JALR 등의 덧셈.
            0b000 => Some(Self::Add),

            // 조건 분기 비교.
            0b001 => match funct3 {
                0x0 | 0x1 => Some(Self::Sub),
                0x4 | 0x5 => Some(Self::Slt),
                0x6 | 0x7 => Some(Self::Sltu),
                _ => None,
            },

            // R-Type 정수·M 확장 연산.
            0b010 => match (funct3, funct7) {
                (0x0, 0x00) => Some(Self::Add),
                (0x0, 0x20) => Some(Self::Sub),

                (0x4, 0x00) => Some(Self::Xor),
                (0x6, 0x00) => Some(Self::Or),
                (0x7, 0x00) => Some(Self::And),

                (0x1, 0x00) => Some(Self::Sll),
                (0x5, 0x00) => Some(Self::Srl),
                (0x5, 0x20) => Some(Self::Sra),

                (0x2, 0x00) => Some(Self::Slt),
                (0x3, 0x00) => Some(Self::Sltu),

                (0x0, 0x01) => Some(Self::Mul),
                (0x1, 0x01) => Some(Self::Mulh),
                (0x2, 0x01) => Some(Self::Mulhsu),
                (0x3, 0x01) => Some(Self::Mulhu),

                (0x4, 0x01) => Some(Self::Div),
                (0x5, 0x01) => Some(Self::Divu),
                (0x6, 0x01) => Some(Self::Rem),
                (0x7, 0x01) => Some(Self::Remu),

                _ => None,
            },

            // I-Type 연산。
            // 일반 즉시값 연산에서는 funct7을 검사하지 않는다.
            // 시프트 즉시값 연산에서만 상위 인코딩을 검사한다.
            0b011 => match funct3 {
                0x0 => Some(Self::Add),
                0x4 => Some(Self::Xor),
                0x6 => Some(Self::Or),
                0x7 => Some(Self::And),

                0x2 => Some(Self::Slt),
                0x3 => Some(Self::Sltu),

                0x1 if funct7 == 0x00 => Some(Self::Sll),
                0x5 if funct7 == 0x00 => Some(Self::Srl),
                0x5 if funct7 == 0x20 => Some(Self::Sra),

                _ => None,
            },

            // LUI.
            0b100 => Some(Self::Lui),

            _ => None,
        };

        operation.ok_or(AluDecodeError {
            alu_op,
            funct3,
            funct7,
        })
    }
}
