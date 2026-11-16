use super::common::{cycle, request};

use super::super::{Alu, AluConfig, AluInputs, AluOperation, AluResult};

#[test]
fn add_can_complete_in_request_cycle() {
    let mut alu = Alu::new();

    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Add, 10, 20)),
            result_ready: true,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(30)));
    assert!(alu.is_idle());
}

#[test]
fn multiply_takes_three_execution_cycles() {
    let mut alu = Alu::new();

    // 사이클 1: 요청 수락.
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

    // 사이클 2: 실행 중.
    let outputs = cycle(&mut alu, AluInputs::default());

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, None);

    // 사이클 3: 결과 제공, 수락은 보류.
    let outputs = cycle(&mut alu, AluInputs::default());

    assert_eq!(outputs.result, Some(AluResult::new(42)));
    assert!(!alu.is_idle());

    // 사이클 4: 다른 요청이 와도 기존 결과 유지.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Add, 100, 200)),
            result_ready: false,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(42)));

    // 사이클 5: 결과 수락.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: None,
            result_ready: true,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(42)));
    assert!(alu.is_idle());
}

#[test]
fn configured_shift_latency_is_preserved() {
    let mut alu = Alu::with_config(AluConfig {
        shift_cycles: 3,
        ..Default::default()
    });

    // 첫 번째 사이클: 요청 수락.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            request: Some(request(AluOperation::Sll, 1, 3)),
            result_ready: true,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, None);
    assert!(!alu.is_idle());

    // 두 번째 사이클: 실행 중.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            result_ready: true,
            ..Default::default()
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, None);

    // 세 번째 사이클: 결과 전달.
    let outputs = cycle(
        &mut alu,
        AluInputs {
            result_ready: true,
            ..Default::default()
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(8)));
    assert!(alu.is_idle());
}

#[test]
fn division_corner_cases() {
    use AluOperation::{Div, Divu, Rem, Remu};

    let cases = [
        (Div, 10, 0, u32::MAX),
        (Divu, 10, 0, u32::MAX),
        (Rem, 10, 0, 10),
        (Remu, 10, 0, 10),
        (Div, i32::MIN as u32, u32::MAX, i32::MIN as u32),
        (Rem, i32::MIN as u32, u32::MAX, 0),
    ];

    // 산술 결과를 검증하기 위해 지연시간은 1로 설정한다.
    let mut alu = Alu::with_config(AluConfig {
        divide_cycles: 1,
        ..Default::default()
    });

    for (operation, a, b, expected) in cases {
        let outputs = cycle(
            &mut alu,
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
            "나눗셈 특수값 처리 불일치: {:?}",
            operation,
        );

        assert!(alu.is_idle());
    }
}
