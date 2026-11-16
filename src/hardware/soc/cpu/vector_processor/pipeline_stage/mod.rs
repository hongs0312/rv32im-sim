use super::components::lane::{Sew, VectorOp};
use super::components::vrf::VLEN_BYTES;

// 💡 1. V-ID 단계의 결과를 담아 V-EX로 넘겨주는 레지스터
#[derive(Clone, Copy, Debug)]
pub struct VIdExRegister {
    pub valid: bool,                // 이 파이프라인 단계에 유효한 명령어가 있는지
    pub op: VectorOp,               // 수행할 연산 종류
    pub sew: Sew,                   // 요소 크기
    pub vs1_data: [u8; VLEN_BYTES], // VRF에서 읽어온 데이터 1
    pub vs2_data: [u8; VLEN_BYTES], // VRF에서 읽어온 데이터 2
    pub vd_idx: usize,              // 결과를 저장할 목적지 레지스터 번호
}

impl Default for VIdExRegister {
    fn default() -> Self {
        Self {
            valid: false,
            op: VectorOp::Add,
            sew: Sew::E32,
            vs1_data: [0; 16],
            vs2_data: [0; 16],
            vd_idx: 0,
        }
    }
}

// 💡 2. V-EX 단계의 연산 결과를 담아 V-MEM으로 넘겨주는 레지스터
#[derive(Clone, Copy, Debug)]
pub struct VExMemRegister {
    pub valid: bool,
    pub alu_result: [u8; VLEN_BYTES], // 벡터 유닛이 계산한 128비트 결과
    pub vd_idx: usize,
}

impl Default for VExMemRegister {
    fn default() -> Self {
        Self {
            valid: false,
            alu_result: [0; 16],
            vd_idx: 0,
        }
    }
}

// 💡 3. V-MEM 단계를 통과한 결과를 V-WB로 넘겨주는 레지스터
#[derive(Clone, Copy, Debug)]
pub struct VMemWbRegister {
    pub valid: bool,
    pub wb_data: [u8; VLEN_BYTES], // 최종적으로 VRF에 쓸 데이터
    pub vd_idx: usize,
}

impl Default for VMemWbRegister {
    fn default() -> Self {
        Self {
            valid: false,
            wb_data: [0; 16],
            vd_idx: 0,
        }
    }
}
