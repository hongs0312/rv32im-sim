use crate::hardware::clocked::Clocked;

use super::common::{cycle, request};

use super::super::{AluInputs, AluOperation, AluResult, divider::DividerUnit};

#[test]
fn division_variants_handle_sign_and_corner_cases() {
    use AluOperation::*;

    let cases = [
        // 부호 있는 몫: 0을 향해 절삭.
        (Div, 7, 3, 2),
        (Div, (-7_i32) as u32, 3, (-2_i32) as u32),
        (Div, 7, (-3_i32) as u32, (-2_i32) as u32),
        (Div, (-7_i32) as u32, (-3_i32) as u32, 2),
        // 나머지의 부호는 피제수와 같다.
        (Rem, 7, 3, 1),
        (Rem, (-7_i32) as u32, 3, u32::MAX),
        (Rem, 7, (-3_i32) as u32, 1),
        (Rem, (-7_i32) as u32, (-3_i32) as u32, u32::MAX),
        // 부호 없는 계산.
        (Divu, u32::MAX, 2, 0x7FFF_FFFF),
        (Remu, u32::MAX, 2, 1),
        // 0으로 나누기.
        (Div, 42, 0, u32::MAX),
        (Divu, 42, 0, u32::MAX),
        (Rem, 42, 0, 42),
        (Remu, 42, 0, 42),
        (Rem, (-7_i32) as u32, 0, (-7_i32) as u32),
        // 부호 있는 나눗셈 오버플로.
        (Div, i32::MIN as u32, u32::MAX, i32::MIN as u32),
        (Rem, i32::MIN as u32, u32::MAX, 0),
        // 0 피제수.
        (Div, 0, 3, 0),
        (Rem, 0, 3, 0),
    ];

    let mut unit = DividerUnit::new(1);

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
            "나눗셈 결과 불일치: {:?}, a={:#010x}, b={:#010x}",
            operation,
            a,
            b,
        );

        assert!(unit.state().is_idle());
    }
}

#[test]
fn evaluate_does_not_modify_current_state() {
    let unit = DividerUnit::new(20);

    let inputs = AluInputs {
        request: Some(request(AluOperation::Div, 100, 4)),
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
    for latency in [1_u8, 2, 10, 20] {
        let mut unit = DividerUnit::new(latency);

        for index in 0..usize::from(latency) {
            let outputs = cycle(
                &mut unit,
                AluInputs {
                    request: Some(if index == 0 {
                        request(AluOperation::Div, 100, 4)
                    } else {
                        // 실행 중 새 요청은 기존 작업에 영향을 주지 않는다.
                        request(AluOperation::Remu, 100, 3)
                    }),
                    result_ready: true,
                    flush: false,
                },
            );

            assert_eq!(outputs.request_ready, index == 0,);

            if index + 1 == usize::from(latency) {
                assert_eq!(outputs.result, Some(AluResult::new(25)),);
            } else {
                assert_eq!(outputs.result, None);
            }
        }

        assert!(unit.state().is_idle());
    }
}

#[test]
fn result_is_held_until_accepted() {
    let mut unit = DividerUnit::new(3);

    for index in 0..3 {
        let outputs = cycle(
            &mut unit,
            AluInputs {
                request: if index == 0 {
                    Some(request(AluOperation::Div, 100, 4))
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
            assert_eq!(outputs.result, Some(AluResult::new(25)),);
        }
    }

    // 결과 대기 중에는 새 요청을 받지 않는다.
    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Remu, 100, 3)),
            result_ready: false,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(25)),);

    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: None,
            result_ready: true,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(25)),);
    assert!(unit.state().is_idle());
}

#[test]
fn flush_discards_running_and_held_results() {
    // 지연시간 1: 결과 대기 상태.
    // 지연시간 3: 실행 중 상태.
    for latency in [1, 3] {
        let mut unit = DividerUnit::new(latency);

        cycle(
            &mut unit,
            AluInputs {
                request: Some(request(AluOperation::Div, 100, 4)),
                result_ready: false,
                flush: false,
            },
        );

        let outputs = cycle(
            &mut unit,
            AluInputs {
                request: Some(request(AluOperation::Remu, 100, 3)),
                result_ready: true,
                flush: true,
            },
        );

        assert!(!outputs.request_ready);
        assert_eq!(outputs.result, None);
        assert!(unit.state().is_idle());
    }
}
