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

    pub is_active: bool, // 현재 사이클에서 PE가 활성화되어 연산을 수행하는지 여부
}

impl ProcessingElement {
    pub const fn new() -> Self {
        Self {
            psum: 0,
            a_reg: StreamValue {
                value: 0,
                valid: false,
            },
            b_reg: StreamValue {
                value: 0,
                valid: false,
            },
            is_active: false,
        }
    }

    // 매 클럭마다 실행되는 PE 연산
    pub fn step(&mut self, a_in: StreamValue, b_in: StreamValue) {
        self.is_active = a_in.valid && b_in.valid;

        if self.is_active {
            let product = (a_in.value as i32).wrapping_mul(b_in.value as i32);
            self.psum = self.psum.wrapping_add(product);
        }

        self.a_reg = a_in;
        self.b_reg = b_in;
    }

    pub fn clear(&mut self) {
        self.psum = 0;
        self.a_reg = StreamValue {
            value: 0,
            valid: false,
        };
        self.b_reg = StreamValue {
            value: 0,
            valid: false,
        };
        self.is_active = false;
    }
}

#[test]
fn pe_accumulates_product() {
    // PE 연산 테스트
    let mut pe = ProcessingElement::default();

    pe.step(
        StreamValue {
            value: 2,
            valid: true,
        },
        StreamValue {
            value: 3,
            valid: true,
        },
    ); // 2 * 3 = 6
    pe.step(
        StreamValue {
            value: 4,
            valid: true,
        },
        StreamValue {
            value: 5,
            valid: true,
        },
    ); // 4 * 5 = 20, 누적 합계 = 6 + 20 = 26
    pe.step(
        StreamValue {
            value: 0,
            valid: false,
        },
        StreamValue {
            value: 0,
            valid: false,
        },
    ); // 유효하지 않은 입력, psum은 변하지 않음

    assert_eq!(pe.psum, 26);
    assert_eq!(pe.is_active, false);
}
