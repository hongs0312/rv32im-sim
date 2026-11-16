//! 곱셈 실행 유닛.
//!
//! 지원 연산:
//! - MUL: 하위 32비트
//! - MULH: 부호 있음 × 부호 있음, 상위 32비트
//! - MULHSU: 부호 있음 × 부호 없음, 상위 32비트
//! - MULHU: 부호 없음 × 부호 없음, 상위 32비트
//!
//! 현재는 지연시간 기반 모델이다.
//! 요청을 저장하고 설정된 실행 시간이 지난 후 결과를 제공한다.

use crate::hardware::clocked::Clocked;

use super::types::{AluInputs, AluOperation, AluOutputs, AluRequest, AluResult, UnitKind};

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MultiplierState {
    Idle,
    Running { cycles_left: u8, request: AluRequest },
    ResultReady { result: AluResult },
}

impl MultiplierState {
    pub(super) fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}

pub(super) struct MultiplierUnit {
    latency: u8,
    state: MultiplierState,
}

impl MultiplierUnit {
    pub(super) fn new(latency: u8) -> Self {
        assert!(latency >= 1, "곱셈기의 지연시간은 1 이상이어야 합니다.");

        Self {
            latency,
            state: MultiplierState::Idle,
        }
    }

    pub(super) fn state(&self) -> MultiplierState {
        self.state
    }

    /// 상태와 시간을 변경하지 않는 곱셈 계산.
    fn calculate_result(request: AluRequest) -> AluResult {
        let AluRequest { operation, a, b } = request;

        let value = match operation {
            AluOperation::Mul => {
                // 하위 32비트는 부호 해석과 관계없이 동일하다.
                a.wrapping_mul(b)
            }

            AluOperation::Mulh => {
                // 두 피연산자를 각각 부호 확장한다.
                let lhs = a as i32 as i64;
                let rhs = b as i32 as i64;
                let product = lhs * rhs;

                (product >> 32) as u32
            }

            AluOperation::Mulhsu => {
                // 첫 번째 피연산자만 부호 확장한다.
                //
                // u32 → i64 변환은 원래의 양수 값을 보존한다.
                // 이 조합의 전체 곱은 i64 범위에 들어간다.
                let lhs = a as i32 as i64;
                let rhs = b as i64;
                let product = lhs * rhs;

                (product >> 32) as u32
            }

            AluOperation::Mulhu => {
                let lhs = a as u64;
                let rhs = b as u64;
                let product = lhs * rhs;

                (product >> 32) as u32
            }

            _ => unreachable!("곱셈기에 지원하지 않는 연산이 전달되었습니다."),
        };

        AluResult::new(value)
    }

    /// 완료 결과를 전달하거나 다음 사이클까지 보관한다.
    fn finish(
        result: AluResult,
        result_ready: bool,
        request_ready: bool,
    ) -> (AluOutputs, MultiplierState) {
        let next = if result_ready {
            MultiplierState::Idle
        } else {
            MultiplierState::ResultReady { result }
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

impl Clocked for MultiplierUnit {
    type Inputs = AluInputs;
    type Outputs = AluOutputs;
    type NextState = MultiplierState;

    fn evaluate(&self, inputs: &Self::Inputs) -> (Self::Outputs, Self::NextState) {
        // 취소는 요청 수락과 결과 전달보다 우선한다.
        if inputs.flush {
            return (AluOutputs::default(), MultiplierState::Idle);
        }

        match self.state {
            MultiplierState::Idle => {
                let Some(request) = inputs.request else {
                    return (
                        AluOutputs {
                            request_ready: true,
                            result: None,
                        },
                        MultiplierState::Idle,
                    );
                };

                assert_eq!(
                    request.operation.unit(),
                    UnitKind::Multiply,
                    "곱셈기로 잘못된 요청이 전달되었습니다."
                );

                if self.latency == 1 {
                    let result = Self::calculate_result(request);

                    // 지연시간 1이면 요청과 결과 전달이
                    // 같은 사이클에 가능하다.
                    Self::finish(result, inputs.result_ready, true)
                } else {
                    (
                        AluOutputs {
                            request_ready: true,
                            result: None,
                        },
                        MultiplierState::Running {
                            cycles_left: self.latency - 1,
                            request,
                        },
                    )
                }
            }

            MultiplierState::Running {
                cycles_left,
                request,
            } => {
                if cycles_left > 1 {
                    (
                        AluOutputs::default(),
                        MultiplierState::Running {
                            cycles_left: cycles_left - 1,
                            request,
                        },
                    )
                } else {
                    // 현재 입력이 아니라 수락 시 저장한 요청 사용.
                    let result = Self::calculate_result(request);

                    Self::finish(result, inputs.result_ready, false)
                }
            }

            MultiplierState::ResultReady { result } => {
                // 새로운 입력으로 재계산하지 않는다.
                Self::finish(result, inputs.result_ready, false)
            }
        }
    }

    fn commit(&mut self, next: Self::NextState) {
        self.state = next;
    }
}
