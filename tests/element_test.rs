#[cfg(test)]
mod tests {
    use rv32i_sim::hardware::soc::cpu::elements::{alu::Alu, decoder::*};
    use rv32i_sim::hardware::soc::types::OpStatus;

    #[test]
    fn alutest() {
        let mut alu = Alu::new();

        // ==========================================
        // 1. Memory / Load-Store (alu_op = 0b00)
        // ==========================================
        // Base Address + Offset 계산
        assert_eq!(
            alu.execute_with_cycles(0x1000, 4, 0b00, 0, 0),
            OpStatus::Complete((0x1004, false))
        ); // 0x1000 + 4 = 0x1004

        // ==========================================
        // 2. Branch Instructions (alu_op = 0b01)
        // 최종적인 분기 여부는 Mem 단계에서 funct3 와 Zero 플래그를 기반으로 판단
        // 따라서 ALU에서는 단순히 두 레지스터의 차이를 계산하고, Zero 플래그를 설정
        // ==========================================
        // BEQ (Equal)
        assert_eq!(
            alu.execute_with_cycles(5, 5, 0b01, 0x0, 0),
            OpStatus::Complete((0, true))
        ); // 5 == 5 -> 참(1)
        assert_eq!(
            alu.execute_with_cycles(5, 3, 0b01, 0x0, 0),
            OpStatus::Complete((2, false))
        ); // 5 == 3 -> 거짓(0)

        // BNE (Not Equal)
        assert_eq!(
            alu.execute_with_cycles(5, 3, 0b01, 0x1, 0),
            OpStatus::Complete((2, false))
        ); // 5 != 3 -> 참(1)
        assert_eq!(
            alu.execute_with_cycles(5, 5, 0b01, 0x1, 0),
            OpStatus::Complete((0, true))
        ); // 5 != 5 -> 거짓(0)

        // BLT (Less Than - Signed)
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFFC, 10, 0b01, 0x4, 0),
            OpStatus::Complete((1, false))
        ); // -4 < 10 -> 참(1)
        assert_eq!(
            alu.execute_with_cycles(10, 0xFFFFFFFC, 0b01, 0x4, 0),
            OpStatus::Complete((0, true))
        ); // 10 < -4 -> 거짓(0)

        // // BGE (Greater Than or Equal - Signed)
        assert_eq!(
            alu.execute_with_cycles(10, 0xFFFFFFFC, 0b01, 0x5, 0),
            OpStatus::Complete((0, true))
        ); // 10 >= -4 -> 참(1)
        assert_eq!(
            alu.execute_with_cycles(5, 5, 0b01, 0x5, 0),
            OpStatus::Complete((0, true))
        ); // 5 >= 5 -> 참(1)
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFFC, 10, 0b01, 0x5, 0),
            OpStatus::Complete((1, false))
        ); // -4 >= 10 -> 거짓(0)

        // BLTU (Less Than - Unsigned)
        assert_eq!(
            alu.execute_with_cycles(10, 0xFFFFFFFC, 0b01, 0x6, 0),
            OpStatus::Complete((1, false))
        ); // 10 < 0xFFFFFFFC -> 참(1)
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFFC, 10, 0b01, 0x6, 0),
            OpStatus::Complete((0, true))
        ); // 0xFFFFFFFC < 10 -> 거짓(0)

        // BGEU (Greater Than or Equal - Unsigned)
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFFC, 10, 0b01, 0x7, 0),
            OpStatus::Complete((0, true))
        ); // 0xFFFFFFFC >= 10 -> 참(1)
        assert_eq!(
            alu.execute_with_cycles(10, 0xFFFFFFFC, 0b01, 0x7, 0),
            OpStatus::Complete((1, false))
        ); // 10 >= 0xFFFFFFFC -> 거짓(0)

        // ==========================================
        // 3. R-Type Instructions (alu_op = 0b10)
        // ==========================================
        // Test ADD
        assert_eq!(
            alu.execute_with_cycles(10, 20, 0b10, 0x0, 0),
            OpStatus::Complete((30, false))
        ); // 10 + 20 = 30

        // Test SUB (funct7 30번째 비트 = 1 -> 1 << 5 = 0x20)
        assert_eq!(
            alu.execute_with_cycles(20, 10, 0b10, 0x0, 1 << 5),
            OpStatus::Complete((10, false))
        );

        // Test SLL (Shift Left Logical)
        assert_eq!(
            alu.execute_with_cycles(0b101, 2, 0b10, 0x1, 0),
            OpStatus::Complete((20, false))
        ); // 5 << 2 = 20

        // Test SLT (Set Less Than - Signed)
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFFC, 10, 0b10, 0x2, 0),
            OpStatus::Complete((1, false))
        ); // -4 < 10 -> 1
        assert_eq!(
            alu.execute_with_cycles(10, 0xFFFFFFFC, 0b10, 0x2, 0),
            OpStatus::Complete((0, true))
        ); // 10 < -4 -> 0

        // Test SLTU (Set Less Than - Unsigned)
        assert_eq!(
            alu.execute_with_cycles(10, 0xFFFFFFFC, 0b10, 0x3, 0),
            OpStatus::Complete((1, false))
        ); // 10 < 0xFFFFFFFC -> 1

        // Test XOR
        assert_eq!(
            alu.execute_with_cycles(0b1010, 0b1100, 0b10, 0x4, 0),
            OpStatus::Complete((0b0110, false))
        ); // 10 ^ 12 = 6

        // Test SRL (Shift Right Logical)
        assert_eq!(
            alu.execute_with_cycles(0b1000, 2, 0b10, 0x5, 0),
            OpStatus::Complete((2, false))
        ); // 8 >> 2 = 2

        // Test SRA (Shift Right Arithmetic) // 현재 문제
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFFC, 1, 0b10, 0x5, 1 << 5),
            OpStatus::Complete((0xFFFFFFFE, false))
        ); // -4 >> 1 = -1

        // Test OR
        assert_eq!(
            alu.execute_with_cycles(0b1010, 0b0101, 0b10, 0x6, 0),
            OpStatus::Complete((0b1111, false))
        ); // 10 | 5 = 15

        // Test AND
        assert_eq!(
            alu.execute_with_cycles(0b1010, 0b1100, 0b10, 0x7, 0),
            OpStatus::Complete((0b1000, false))
        ); // 10 & 12 = 8

        // ==========================================
        // 4. I-Type Instructions (alu_op = 0b11)
        // ==========================================
        // Test ADDI (양수 즉시값)
        assert_eq!(
            alu.execute_with_cycles(10, 5, 0b11, 0x0, 0),
            OpStatus::Complete((15, false))
        ); // 10 + 5 = 15

        // Test ADDI (음수 즉시값: -4 / 0xFFFFFFFC)
        assert_eq!(
            alu.execute_with_cycles(10, 0xFFFFFFFC, 0b11, 0x0, 0),
            OpStatus::Complete((6, false))
        ); // 10 + (-4) = 6

        // Test SLLI
        assert_eq!(
            alu.execute_with_cycles(1, 3, 0b11, 0x1, 0),
            OpStatus::Complete((8, false))
        ); // 1 << 3 = 8

        // Test SLTI (Signed)
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFFC, 10, 0b11, 0x2, 0),
            OpStatus::Complete((1, false))
        ); // -4 < 10 -> 1

        // Test SLTIU (Unsigned)
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFFC, 10, 0b11, 0x3, 0),
            OpStatus::Complete((0, true))
        ); // 0xFFFFFFFC < 10 -> 0

        // Test XORI
        assert_eq!(
            alu.execute_with_cycles(0b1010, 0b0101, 0b11, 0x4, 0),
            OpStatus::Complete((15, false))
        ); // 10 ^ 5 = 15

        // Test SRLI
        assert_eq!(
            alu.execute_with_cycles(16, 2, 0b11, 0x5, 0),
            OpStatus::Complete((4, false))
        ); // 16 >> 2 = 4

        // Test SRAI
        assert_eq!(
            alu.execute_with_cycles(0xFFFFFFF0, 2, 0b11, 0x5, 1 << 5),
            OpStatus::Complete((0xFFFFFFFC, false))
        ); // -16 >> 2 = -4

        // Test ORI
        assert_eq!(
            alu.execute_with_cycles(10, 5, 0b11, 0x6, 0),
            OpStatus::Complete((15, false))
        ); // 10 | 5 = 15

        // Test ANDI
        assert_eq!(
            alu.execute_with_cycles(10, 12, 0b11, 0x7, 0),
            OpStatus::Complete((8, false))
        ); // 10 & 12 = 8

        println!("All ALU tests passed successfully!");
    }

    #[test]
    fn imm_gen_test() {
        // Test I-Type
        let inst_i = 0b000000000001_00000_000_00000_0010011; // ADDI x0, x0, 1
        assert_eq!(Decoder::imm_gen(inst_i), 1);

        // Test S-Type
        let inst_s = 0b0000000_00001_00000_010_00000_0100011; // SW x1, 0(x0)
        assert_eq!(Decoder::imm_gen(inst_s), 0);

        // Test B-Type
        let inst_b = 0b0000000_00001_00000_000_00000_1100011; // BEQ x0, x1, 0
        assert_eq!(Decoder::imm_gen(inst_b), 0);
    }

    #[test]
    fn register_file_test() {
        let mut regs = rv32i_sim::hardware::soc::cpu::elements::register::RegisterFile::new();

        // Test writing to a register
        regs.write(1, 42, true);
        assert_eq!(regs.read(1), 42);

        // Test reading from x0 (should always be 0)
        assert_eq!(regs.read(0), 0);

        // Test that writing to x0 does not change its value
        regs.write(0, 100, true);
        assert_eq!(regs.read(0), 0);
    }
}
