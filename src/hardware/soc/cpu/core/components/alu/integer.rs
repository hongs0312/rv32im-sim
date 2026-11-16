//! 정수 실행 유닛.
//!
//! 지원 연산:
//! ADD, SUB, AND, OR, XOR, SLT, SLTU, LUI
//!
//! 전달 규칙:
//! - 요청 수락 시 연산과 피연산자를 보관한다.
//! - 결과가 수락되지 않으면 계속 보존한다.
//! - 실행 중에는 새로운 요청을 받지 않는다.
//! - flush는 요청·결과 전달보다 우선한다.

use crate::hardware::clocked::Clocked;

use super::types::{AluInputs, AluOperation, AluOutputs, AluRequest, AluResult, UnitKind};

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerState {
    Idle,
    Running { cycles_left: u8, request: AluRequest },
    ResultReady { result: AluResult },
}

impl IntegerState {
    pub(super) fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}

pub(super) struct IntegerUnit {
    latency: u8,
    state: IntegerState,
}

impl IntegerUnit {
    pub(super) fn new(latency: u8) -> Self {
        assert!(
            latency >= 1,
            "정수 실행 유닛의 지연시간은 1 이상이어야 합니다."
        );

        Self {
            latency,
            state: IntegerState::Idle,
        }
    }

    pub(super) fn state(&self) -> IntegerState {
        self.state
    }

    /// 순수 산술 계산. 상태나 시간을 변경하지 않는다.
    fn calculate_result(request: AluRequest) -> AluResult {
        let AluRequest { operation, a, b } = request;

        let value = match operation {
            AluOperation::Add => a.wrapping_add(b),
            AluOperation::Sub => a.wrapping_sub(b),

            AluOperation::And => a & b,
            AluOperation::Or => a | b,
            AluOperation::Xor => a ^ b,

            AluOperation::Slt => u32::from((a as i32) < (b as i32)),

            AluOperation::Sltu => u32::from(a < b),

            AluOperation::Lui => b,

            _ => unreachable!("정수 실행 유닛에 지원하지 않는 연산이 전달되었습니다."),
        };

        AluResult::new(value)
    }

    /// 완료 결과와 수락 여부로 출력 및 다음 상태를 계산한다.
    fn finish(
        result: AluResult,
        result_ready: bool,
        request_ready: bool,
    ) -> (AluOutputs, IntegerState) {
        let next = if result_ready {
            IntegerState::Idle
        } else {
            IntegerState::ResultReady { result }
        };

        (
            AluOutputs {
                request_ready,
                result: Some(result),
            },
            next,
        )
    }
}

impl Clocked for IntegerUnit {
    type Inputs = AluInputs;
    type Outputs = AluOutputs;
    type NextState = IntegerState;

    fn evaluate(&self, inputs: &Self::Inputs) -> (Self::Outputs, Self::NextState) {
        // 취소 사이클에는 요청·결과 전달을 모두 차단한다.
        if inputs.flush {
            return (AluOutputs::default(), IntegerState::Idle);
        }

        match self.state {
            IntegerState::Idle => {
                let Some(request) = inputs.request else {
                    return (
                        AluOutputs {
                            request_ready: true,
                            result: None,
                        },
                        IntegerState::Idle,
                    );
                };

                assert_eq!(
                    request.operation.unit(),
                    UnitKind::Integer,
                    "정수 실행 유닛으로 잘못된 요청이 전달되었습니다."
                );

                if self.latency == 1 {
                    let result = Self::calculate_result(request);

                    // 요청 수락과 결과 전달이 같은 사이클에 가능하다.
                    Self::finish(result, inputs.result_ready, true)
                } else {
                    (
                        AluOutputs {
                            request_ready: true,
                            result: None,
                        },
                        IntegerState::Running {
                            cycles_left: self.latency - 1,
                            request,
                        },
                    )
                }
            }

            IntegerState::Running {
                cycles_left,
                request,
            } => {
                if cycles_left > 1 {
                    (
                        AluOutputs::default(),
                        IntegerState::Running {
                            cycles_left: cycles_left - 1,
                            request,
                        },
                    )
                } else {
                    let result = Self::calculate_result(request);

                    Self::finish(result, inputs.result_ready, false)
                }
            }

            IntegerState::ResultReady { result } => {
                // 새 입력으로 재계산하지 않고 기존 결과를 유지한다.
                Self::finish(result, inputs.result_ready, false)
            }
        }
    }

    fn commit(&mut self, next: Self::NextState) {
        self.state = next;
    }
}
