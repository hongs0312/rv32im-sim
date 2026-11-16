use crate::hardware::clocked::Clocked;

use super::common::{cycle, request};

use super::super::{AluInputs, AluOperation, AluResult, integer::IntegerUnit};

#[test]
fn add_completes_in_one_cycle() {
    let mut unit = IntegerUnit::new(1);

    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Add, 10, 20)),
            result_ready: true,
            flush: false,
        },
    );

    assert!(outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(30)));
    assert!(unit.state().is_idle());
}

#[test]
fn evaluate_does_not_modify_current_state() {
    let unit = IntegerUnit::new(3);

    let inputs = AluInputs {
        request: Some(request(AluOperation::Add, 10, 20)),
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
fn result_is_held_until_accepted() {
    let mut unit = IntegerUnit::new(1);

    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Add, 10, 20)),
            result_ready: false,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(30)));

    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: Some(request(AluOperation::Sub, 100, 1)),
            result_ready: false,
            flush: false,
        },
    );

    assert!(!outputs.request_ready);
    assert_eq!(outputs.result, Some(AluResult::new(30)));

    let outputs = cycle(
        &mut unit,
        AluInputs {
            request: None,
            result_ready: true,
            flush: false,
        },
    );

    assert_eq!(outputs.result, Some(AluResult::new(30)));
    assert!(unit.state().is_idle());
}
