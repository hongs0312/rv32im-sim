use crate::hardware::clocked::Clocked;

use super::common::{cycle, request};

use super::super::{AluInputs, AluOperation, AluResult, shifter::ShifterUnit};

#[test]
fn shift_operations_handle_sign_and_shift_amount() {
    use AluOperation::*;

    let cases = [
        // 일반 시프트.
        (Sll, 5, 2, 20),
        (Srl, 16, 2, 4),
        (Sra, (-4_i32) as u32, 1, (-2_i32) as u32),
        // 논리 오른쪽 시프트는 상위 비트를 0으로 채운다.
        (Srl, 0xFFFF_FFFC, 1, 0x7FFF_FFFE),
        // 왼쪽으로 밀려난 상위 비트는 버려진다.
        (Sll, 0x8000_0000, 1, 0),
        // 시프트 양은 하위 5비트만 사용한다.
        (Sll, 1, 32, 1),
        (Sll, 1, 33, 2),
        (Sll, 1, u32::MAX, 0x8000_0000),
        (Srl, 0x8000_0000, 31, 1),
        (Sra, 0x8000_0000, u32::MAX, u32::MAX),
        // 0비트 시프트.
        (Sra, 0x8000_0000, 0, 0x8000_0000),
    ];

    let mut unit = ShifterUnit::new(1);

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
            "시프트 결과 불일치: {:?}, a={:#010x}, b={}",
            operation,
            a,
            b,
        );
        assert!(unit.state().is_idle());
    }
}

#[test]
fn evaluate_does_not_modify_current_state() {
    let unit = ShifterUnit::new(3);

    let inputs = AluInputs {
        request: Some(request(AluOperation::Sll, 1, 3)),
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
fn configured_latency_uses_the_latched_request() {
    let mut unit = ShifterUnit::new(3);

    // 첫 번째 사이클: SLL 요청 수락.
    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Sll, 1, 3)),
            result_ready: true,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, None);

    // 두 번째 사이클: 다른 요청은 무시한다.
    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Sra, u32::MAX, 31)),
            result_ready: true,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, None);

    // 세 번째 사이클: 최초 요청의 결과가 나와야 한다.
    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Srl, 100, 1)),
            result_ready: true,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(8)));
    assert!(unit.state().is_idle());
}

#[test]
fn result_is_held_until_accepted() {
    let mut unit = ShifterUnit::new(1);
    let expected = AluResult::new((-2_i32) as u32);

    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Sra, (-4_i32) as u32, 1)),
            result_ready: false,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(expected));

    // 결과 대기 중에는 새 요청을 수락하지 않는다.
    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Sll, 1, 10)),
            result_ready: false,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(expected));

    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: None,
            result_ready: true,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(expected));
    assert!(unit.state().is_idle());
}

#[test]
fn flush_discards_running_and_held_results() {
    // 지연시간 1은 결과 대기, 3은 실행 중 상태를 검증한다.
    for latency in [1, 3] {
        let mut unit = ShifterUnit::new(latency);

        cycle(
            &mut unit,
            AluInputs {
                request: Some(request(AluOperation::Sll, 1, 3)),
                result_ready: false,
                flush: false,
            },
        );

        let outputs = cycle(
            &mut unit,
            AluInputs {
                request: Some(request(AluOperation::Srl, 100, 1)),
                result_ready: true,
                flush: true,
            },
        );

        assert!(!outputs.request_ready);
        assert_eq!(outputs.result, None);
        assert!(unit.state().is_idle());
    }
}
