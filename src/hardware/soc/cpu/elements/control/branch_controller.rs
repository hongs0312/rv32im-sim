pub struct BranchController;

impl BranchController {
    pub fn get_branch_condition(alu_out: u32, zero: bool, funct3: u8) -> bool {
        let f0 = (funct3 & 0b001) != 0; // funct3의 비트 0 invert 여부 확인
        let f2 = (funct3 & 0b100) != 0; // funct3의 비트 2 

        let l = (alu_out & 1) == 1; // ALU 결과의 최하위 비트 확인 slt/sltu 연산 결과

        let signal = if f2 { l } else { zero }; // f2가 1이면 slt/sltu 연산 결과를 사용, 아니면 zero 플래그 사용

        signal ^ f0 // f0이 1이면 결과를 반전
    }
}
