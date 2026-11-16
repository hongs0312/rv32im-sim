use crate::hardware::clocked::Clocked;

use super::common::{cycle, request};

use super::super::{AluInputs, AluOperation, AluResult, multiplier::MultiplierUnit};

#[test]
fn multiplication_variants_handle_sign_and_high_bits() {
    use AluOperation::*;

    let cases = [
        // 하위 32비트.
        (Mul, 6, 7, 42),
        (Mul, u32::MAX, u32::MAX, 1),
        (Mul, 0x8000_0000, 2, 0),
        // 부호 있음 × 부호 있음.
        (Mulh, u32::MAX, 2, u32::MAX),
        (Mulh, u32::MAX, u32::MAX, 0),
        (Mulh, 0x8000_0000, 2, u32::MAX),
        (Mulh, 0x8000_0000, 0x8000_0000, 0x4000_0000),
        // 부호 있음 × 부호 없음.
        (Mulhsu, u32::MAX, u32::MAX, u32::MAX),
        (Mulhsu, 0x8000_0000, u32::MAX, 0x8000_0000),
        (Mulhsu, 0x7FFF_FFFF, u32::MAX, 0x7FFF_FFFE),
        // 부호 없음 × 부호 없음.
        (Mulhu, u32::MAX, u32::MAX, 0xFFFF_FFFE),
        (Mulhu, 0x8000_0000, 2, 1),
        (Mulhu, 0x8000_0000, 0x8000_0000, 0x4000_0000),
    ];

    let mut unit = MultiplierUnit::new(1);

    for (operation, a, b, expected) in cases {
        let outputs = cycle(
            &mut unit,
            AluInputs {
                request: Some(request(operation, a, b)),
                result_ready: true,
                flush: false,
            },
        );

        assert!(outputs.request_ready);

        assert_eq!(
            outputs.result,
            Some(AluResult::new(expected)),
            "곱셈 결과 불일치: {:?}, a={:#010x}, b={:#010x}",
            operation,
            a,
            b,
        );

        assert!(unit.state().is_idle());
    }
}

#[test]
fn evaluate_does_not_modify_current_state() {
    let unit = MultiplierUnit::new(3);

    let inputs = AluInputs {
        request: Some(request(AluOperation::Mul, 6, 7)),
        result_ready: false,
        flush: false,
    };

    let before = unit.state();

    let (first_output, first_next) = unit.evaluate(&inputs);
    let (second_output, second_next) = unit.evaluate(&inputs);

    assert_eq!(unit.state(), before);
    assert_eq!(first_output, second_output);
    assert_eq!(first_next, second_next);
}

#[test]
fn configured_latency_and_latched_request_are_preserved() {
    for latency in [1_u8, 2, 3, 5] {
        let mut unit = MultiplierUnit::new(latency);

        for index in 0..usize::from(latency) {
            let inputs = AluInputs {
                request: Some(if index == 0 {
                    request(AluOperation::Mul, 6, 7)
                } else {
                    // 실행 중인 작업에 영향을 주면 안 된다.
                    request(AluOperation::Mulhu, u32::MAX, u32::MAX)
                }),
                result_ready: true,
                flush: false,
            };

            let outputs = cycle(&mut unit, inputs);

            assert_eq!(outputs.request_ready, index == 0,);

            if index + 1 == usize::from(latency) {
                assert_eq!(outputs.result, Some(AluResult::new(42)),);
            } else {
                assert_eq!(outputs.result, None);
            }
        }

        assert!(unit.state().is_idle());
    }
}

#[test]
fn result_is_held_until_accepted() {
    let mut unit = MultiplierUnit::new(3);

    for index in 0..3 {
        let outputs = cycle(
            &mut unit,
            AluInputs {
                request: if index == 0 {
                    Some(request(AluOperation::Mul, 6, 7))
                } else {
                    None
                },
                result_ready: false,
                flush: false,
            },
        );

        if index < 2 {
            assert_eq!(outputs.result, None);
        } else {
            assert_eq!(outputs.result, Some(AluResult::new(42)),);
        }
    }

    // 결과가 준비된 뒤에도 새 요청은 받지 않는다.
    for _ in 0..3 {
        let outputs = cycle(
            &mut unit,
            AluInputs {
                request: Some(request(AluOperation::Mul, 100, 200)),
                result_ready: false,
                flush: false,
            },
        );

        assert!(!outputs.request_ready);
        assert_eq!(outputs.result, Some(AluResult::new(42)),);
    }

    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: None,
            result_ready: true,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(42)),);
    assert!(unit.state().is_idle());
}

#[test]
fn flush_discards_running_and_held_results() {
    // 지연시간 1: 결과 대기 상태.
    // 지연시간 3: 실행 중 상태.
    for latency in [1, 3] {
        let mut unit = MultiplierUnit::new(latency);

        cycle(
            &mut unit,
            AluInputs {
                request: Some(request(AluOperation::Mul, 6, 7)),
                result_ready: false,
                flush: false,
            },
        );

        let outputs = cycle(
            &mut unit,
            AluInputs {
                request: Some(request(AluOperation::Mul, 100, 200)),
                result_ready: true,
                flush: true,
            },
        );

        assert!(!outputs.request_ready);
        assert_eq!(outputs.result, None);
        assert!(unit.state().is_idle());
    }
}
