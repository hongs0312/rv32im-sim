//! 나눗셈·나머지 실행 유닛.
//!
//! 지원 연산:
//! - DIV: 부호 있는 나눗셈
//! - DIVU: 부호 없는 나눗셈
//! - REM: 부호 있는 나머지
//! - REMU: 부호 없는 나머지
//!
//! 현재는 지연시간 기반 모델이다.
//! 요청을 저장하고 설정된 실행 시간이 지난 후 결과를 제공한다.
//!
//! 0으로 나누기와 부호 있는 나눗셈 오버플로는
//! 트랩이 아니라 정의된 결과값으로 처리한다.

use crate::hardware::clocked::Clocked;

use super::types::{AluInputs, AluOperation, AluOutputs, AluRequest, AluResult, UnitKind};

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DividerState {
    Idle,
    Running { cycles_left: u8, request: AluRequest },
    ResultReady { result: AluResult },
}

impl DividerState {
    pub(super) fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}

pub(super) struct DividerUnit {
    latency: u8,
    state: DividerState,
}

impl DividerUnit {
    pub(super) fn new(latency: u8) -> Self {
        assert!(latency >= 1, "나눗셈기의 지연시간은 1 이상이어야 합니다.");

        Self {
            latency,
            state: DividerState::Idle,
        }
    }

    pub(super) fn state(&self) -> DividerState {
        self.state
    }

    /// 부호 있는 몫과 나머지를 계산한다.
    ///
    /// 일반적인 몫은 0을 향해 절삭한다.
    /// 0이 아닌 나머지의 부호는 피제수와 같다.
    fn calculate_signed(a: u32, b: u32) -> (u32, u32) {
        let dividend = a as i32;
        let divisor = b as i32;

        if divisor == 0 {
            // 몫은 모든 비트가 1, 나머지는 피제수.
            (u32::MAX, a)
        } else if dividend == i32::MIN && divisor == -1 {
            // 표현 가능한 범위를 넘는 유일한 부호 있는 나눗셈.
            (i32::MIN as u32, 0)
        } else {
            ((dividend / divisor) as u32, (dividend % divisor) as u32)
        }
    }

    /// 부호 없는 몫과 나머지를 계산한다.
    fn calculate_unsigned(a: u32, b: u32) -> (u32, u32) {
        if b == 0 {
            (u32::MAX, a)
        } else {
            (a / b, a % b)
        }
    }

    /// 상태와 시간을 변경하지 않는 결과 계산.
    fn calculate_result(request: AluRequest) -> AluResult {
        let AluRequest { operation, a, b } = request;

        let value = match operation {
            AluOperation::Div => Self::calculate_signed(a, b).0,
            AluOperation::Rem => Self::calculate_signed(a, b).1,

            AluOperation::Divu => Self::calculate_unsigned(a, b).0,
            AluOperation::Remu => Self::calculate_unsigned(a, b).1,

            _ => unreachable!("나눗셈기에 지원하지 않는 연산이 전달되었습니다."),
        };

        AluResult::new(value)
    }

    /// 완료 결과를 전달하거나 다음 사이클까지 보존한다.
    fn finish(
        result: AluResult,
        result_ready: bool,
        request_ready: bool,
    ) -> (AluOutputs, DividerState) {
        let next = if result_ready {
            DividerState::Idle
        } else {
            DividerState::ResultReady { result }
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

impl Clocked for DividerUnit {
    type Inputs = AluInputs;
    type Outputs = AluOutputs;
    type NextState = DividerState;

    fn evaluate(&self, inputs: &Self::Inputs) -> (Self::Outputs, Self::NextState) {
        // 취소 사이클에는 요청과 결과 전달을 모두 차단한다.
        if inputs.flush {
            return (AluOutputs::default(), DividerState::Idle);
        }

        match self.state {
            DividerState::Idle => {
                let Some(request) = inputs.request else {
                    return (
                        AluOutputs {
                            request_ready: true,
                            result: None,
                        },
                        DividerState::Idle,
                    );
                };

                assert_eq!(
                    request.operation.unit(),
                    UnitKind::Divide,
                    "나눗셈기로 잘못된 요청이 전달되었습니다."
                );

                if self.latency == 1 {
                    let result = Self::calculate_result(request);

                    // 설정값이 1이면 요청과 결과 전달이
                    // 같은 사이클에 가능하다.
                    Self::finish(result, inputs.result_ready, true)
                } else {
                    (
                        AluOutputs {
                            request_ready: true,
                            result: None,
                        },
                        DividerState::Running {
                            cycles_left: self.latency - 1,
                            request,
                        },
                    )
                }
            }

            DividerState::Running {
                cycles_left,
                request,
            } => {
                if cycles_left > 1 {
                    (
                        AluOutputs::default(),
                        DividerState::Running {
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

            DividerState::ResultReady { result } => {
                // 수락 전에는 같은 결과를 유지한다.
                Self::finish(result, inputs.result_ready, false)
            }
        }
    }

    fn commit(&mut self, next: Self::NextState) {
        self.state = next;
    }
}
