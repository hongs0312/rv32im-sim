use crate::hardware::clocked::Clocked;

use super::common::{cycle, request};

use super::super::{Alu, AluConfig, AluInputs, AluOperation, AluResult};

#[test]
fn evaluate_does_not_modify_current_state() {
    let alu = Alu::new();
    let before = alu.states();

    let inputs = AluInputs {
        request: Some(request(AluOperation::Mul, 6, 7)),
        result_ready: false,
        flush: false,
    };

    let _ = alu.evaluate(&inputs);
    let _ = alu.evaluate(&inputs);

    assert_eq!(alu.states(), before);
    assert!(alu.is_idle());
}

#[test]
fn flush_discards_running_operation() {
    let mut alu = Alu::new();

    cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Div, 100, 4)),
            result_ready: false,
            flush: false,
        },
    );

    // 취소와 함께 들어온 ADD 요청도 수락하면 안 된다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Add, 1, 2)),
            result_ready: true,
            flush: true,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, None);
    assert!(alu.is_idle());

    // 다음 사이클의 새 요청은 정상적으로 실행한다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Add, 1, 2)),
            result_ready: true,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(3)));
}

#[test]
fn held_shift_result_blocks_new_integer_request() {
    let mut alu = Alu::new();

    // 쉬프트 결과를 생성하되 수락하지 않는다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Sll, 1, 3)),
            result_ready: false,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(8)));

    let add_request = request(AluOperation::Add, 10, 20);

    // 기존 결과 수락과 새 요청 수락을 동시에 하지 않는다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(add_request),
            result_ready: true,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(8)));
    assert!(alu.is_idle());

    // 다음 사이클에 같은 ADD 요청을 다시 제시한다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(add_request),
            result_ready: true,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(30)));
    assert!(alu.is_idle());
}

#[test]
fn flush_discards_shift_result_and_allows_new_request() {
    let mut alu = Alu::new();

    cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Srl, 16, 2)),
            result_ready: false,
            flush: false,
        },
    );

    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: None,
            result_ready: true,
            flush: true,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, None);
    assert!(alu.is_idle());

    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Sub, 20, 10)),
            result_ready: true,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(10)));
    assert!(alu.is_idle());
}

#[test]
fn multiplier_preserves_latency_and_blocks_new_requests() {
    let mut alu = Alu::with_config(AluConfig {
        multiply_cycles: 2,
        ..Default::default()
    });

    // 첫 번째 사이클: 곱셈 요청 수락.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Mul, 6, 7)),
            result_ready: false,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, None);

    // 두 번째 사이클: 결과 준비, 수락은 보류.
    let outputs = cycle(&mut alu, AluInputs::default());

    assert_eq!(outputs.result, Some(AluResult::new(42)));
    assert!(!alu.is_idle());

    let add_request = request(AluOperation::Add, 10, 20);

    // 결과 대기 중에는 새 요청을 수락하지 않는다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(add_request),
            result_ready: false,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(42)));

    // 결과 수락 사이클에도 새 요청은 수락하지 않는다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(add_request),
            result_ready: true,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(42)));
    assert!(alu.is_idle());

    // 다음 사이클에 ADD 요청을 다시 제시한다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(add_request),
            result_ready: true,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(30)));
}

#[test]
fn flush_cancels_multiplier_before_new_integer_operation() {
    // 지연시간 1: 결과 대기 상태.
    // 지연시간 3: 실행 중 상태.
    for multiply_cycles in [1, 3] {
        let mut alu = Alu::with_config(AluConfig {
            multiply_cycles,
            ..Default::default()
        });

        cycle(
            &mut alu,
            AluInputs {
                request: Some(request(AluOperation::Mulhu, u32::MAX, u32::MAX)),
                result_ready: false,
                flush: false,
            },
        );

        let outputs = cycle(
            &mut alu,
            AluInputs {
                request: None,
                result_ready: true,
                flush: true,
            },
        );

        assert!(!outputs.request_ready);
        assert_eq!(outputs.result, None);
        assert!(alu.is_idle());

        let outputs = cycle(
            &mut alu,
            AluInputs {
                request: Some(request(AluOperation::Add, 10, 20)),
                result_ready: true,
                flush: false,
            },
        );

        assert_eq!(outputs.result, Some(AluResult::new(30)));
        assert!(alu.is_idle());
    }
}

#[test]
fn held_division_result_blocks_new_integer_request() {
    let mut alu = Alu::with_config(AluConfig {
        divide_cycles: 2,
        ..Default::default()
    });

    // 첫 번째 사이클: 나눗셈 요청 수락.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Div, 100, 4)),
            result_ready: false,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, None);

    // 두 번째 사이클: 결과 준비, 수락은 보류.
    let outputs = cycle(&mut alu, AluInputs::default());

    assert_eq!(outputs.result, Some(AluResult::new(25)));
    assert!(!alu.is_idle());

    let add_request = request(AluOperation::Add, 10, 20);

    // 기존 결과 수락과 새 요청 수락을 동시에 하지 않는다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(add_request),
            result_ready: true,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(25)));
    assert!(alu.is_idle());

    // 다음 사이클에 ADD 요청을 다시 제시한다.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(add_request),
            result_ready: true,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(30)));
}

#[test]
fn flush_cancels_divider_and_allows_new_operation() {
    // 지연시간 1: 결과 대기 상태.
    // 지연시간 3: 실행 중 상태.
    for divide_cycles in [1, 3] {
        let mut alu = Alu::with_config(AluConfig {
            divide_cycles,
            ..Default::default()
        });

        cycle(
            &mut alu,
            AluInputs {
                request: Some(request(AluOperation::Div, 100, 4)),
                result_ready: false,
                flush: false,
            },
        );

        let outputs = cycle(
            &mut alu,
            AluInputs {
                request: None,
                result_ready: true,
                flush: true,
            },
        );

        assert!(!outputs.request_ready);
        assert_eq!(outputs.result, None);
        assert!(alu.is_idle());

        let outputs = cycle(
            &mut alu,
            AluInputs {
                request: Some(request(AluOperation::Sub, 20, 10)),
                result_ready: true,
                flush: false,
            },
        );

        assert_eq!(outputs.result, Some(AluResult::new(10)));
        assert!(alu.is_idle());
    }
}
