/*
    systolic array의 processing element를 구현한 모듈
    ProcessingElement는 A와 B 데이터를 입력받아 곱셈을 수행하고, 결과를 누적하여 psum에 저장합니다.
    또한, A와 B 데이터를 다음 PE로 전달하기 위해 a_reg과 b_reg에 저장합니다.
*/
use super::StreamValue;

#[derive(Clone, Copy, Default)]
pub struct ProcessingElement {
    pub psum: i32, // OS 방식: PE 내부에 고정되어 누적되는 결과 (Accumulator)

    pub a_reg: StreamValue, // A 데이터 레지스터
    pub b_reg: StreamValue, // B 데이터 레지스터

    pub mul_reg: i32, // 곱셈 결과 레지스터 (곱셈 결과를 다음 PE로 전달하기 위해 사용)
    pub mul_valid: bool, // 곱셈 결과가 유효한지 여부

    pub is_active: bool, // 현재 사이클에서 PE가 활성화되어 연산을 수행하는지 여부
}

impl ProcessingElement {
    #[rustfmt::skip]
    pub const fn new() -> Self {
        Self {
            psum: 0,
            a_reg: StreamValue { value: 0, valid: false },
            b_reg: StreamValue { value: 0, valid: false },

            mul_reg: 0,
            mul_valid: false,

            is_active: false,
        }
    }

    // 매 클럭마다 실행되는 PE 연산
    // 수정사항: pe에 파이프라이닝 구조로 변경하여 2사이클에 걸쳐 연산하도록 변경
    pub fn step(&mut self, a_in: StreamValue, b_in: StreamValue) {
        // stage 2: 이전 사이클의 곱셈 결과를 PSUM에 누적
        let prev_mul_valid = self.mul_valid;
        if prev_mul_valid {
            self.psum = self.psum.wrapping_add(self.mul_reg);
        }

        // stage 1: 현재 입력 A와 B를 곱셈하여 mul_reg에 저장
        let current_input_valid = a_in.valid && b_in.valid;
        if current_input_valid {
            self.mul_reg = (a_in.value as i32).wrapping_mul(b_in.value as i32);
            self.mul_valid = true;
        } else {
            self.mul_valid = false;
        }

        // 이웃 PE로 전달하기 위해 현재 입력 A와 B를 레지스터에 저장
        // 현재 pe에서 계산이 끝나지 않더라도 값은 주위 PE로 전달
        self.a_reg = a_in;
        self.b_reg = b_in;

        // 곱셈중이거나 현재 입력이 유효하면 PE는 활성화 상태로 간주
        self.is_active = current_input_valid || prev_mul_valid;
    }

    #[rustfmt::skip]
    pub fn clear(&mut self) {
        self.psum = 0;
        self.a_reg = StreamValue { value: 0, valid: false };
        self.b_reg = StreamValue { value: 0, valid: false };

        self.mul_reg = 0;
        self.mul_valid = false;

        self.is_active = false;
    }
}

#[test]
#[rustfmt::skip]
fn pe_accumulates_product() {
    // PE 연산 테스트
    let mut pe = ProcessingElement::default();

    // Cycle 1: 입력 A=2, B=3 -> 곱셈 결과 6, psum은 아직 0
    pe.step(
        StreamValue { value: 2, valid: true },
        StreamValue { value: 3, valid: true },
    );
    assert_eq!(pe.psum, 0);

    // Cycle 2: 입력 A=4, B=5 -> 이전 곱셈 결과 6을 psum에 누적 곱셈 결과 20
    pe.step(
        StreamValue { value: 4, valid: true },
        StreamValue { value: 5, valid: true },
    );
    assert_eq!(pe.psum, 6);

    // Cycle 3: 입력 A=1, B=2 -> 이전 곱셈 결과 20을 psum에 누적 곱셈 결과 26
    pe.step(
        StreamValue { value: 0, valid: false },
        StreamValue { value: 0, valid: false },
    );
    assert_eq!(pe.psum, 26);
    assert_eq!(pe.is_active, true);

    // Cycle 4: 입력 A=0, B=0 -> 이전 곱셈 결과 0을 psum에 누적, psum은 변하지 않음
    pe.step(
        StreamValue { value: 0, valid: false },
        StreamValue { value: 0, valid: false },
    );
    assert_eq!(pe.is_active, false);
}
