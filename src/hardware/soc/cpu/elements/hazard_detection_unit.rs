/*
    memory hazard 해결을 위한 hazard detection unit구현
    - hazard detection unit은 EX 단계에서 발생하는 memory hazard를 해결하기 위해 사용됨
    - EX 단계에서 load 명령어가 수행될 때, 다음 명령어가 load 명령어의 결과를 필요로 하는 경우, pipeline을 stall 시켜야 함
*/

use crate::hardware::soc::cpu::elements::decoder::*;

pub struct HazardDetectionUnit;

impl HazardDetectionUnit {
    // load-use hazard detection
    // EX 단계에서 load 명령어가 수행될 때, 다음 명령어가 load 명령어의 결과를 필요로 하는 경우, pipeline을 stall 시켜야 함
    pub fn check_load_use(id_ex_mem_read: bool, id_ex_rd: u8, if_id_instruction: u32) -> bool {
        // EX 단계에서 load 명령어가 아니거나 쓰기 대상 레지스터가 x0이면 hazard 없음
        if !id_ex_mem_read || id_ex_rd == 0 {
            return false;
        }

        // IF 단계에서 명령어를 디코딩하여 rs1, rs2 레지스터를 확인
        let (_, rs2, rs1, _, _, _) = Decoder::decode(if_id_instruction);

        // load-use hazard 발생 여부를 판단
        (rs1 == id_ex_rd) || (rs2 == id_ex_rd)
    }
}
