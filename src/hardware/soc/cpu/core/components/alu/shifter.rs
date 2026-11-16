//! 시프트 실행 유닛.
//!
//! 지원 연산:
//! - SLL: 논리 왼쪽 시프트
//! - SRL: 논리 오른쪽 시프트
//! - SRA: 산술 오른쪽 시프트
//!
//! 현재 구현은 설정된 지연시간을 사용하는 모델이다.
//! 반복형 쉬프터의 내부 비트 이동 과정을 모사하지는 않는다.

use crate::hardware::clocked::Clocked;

use super::types::{AluInputs, AluOperation, AluOutputs, AluRequest, AluResult, UnitKind};

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ShifterState {
    Idle,
    Running { cycles_left: u8, request: AluRequest },
    ResultReady { result: AluResult },
}

impl ShifterState {
    pub(super) fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}

pub(super) struct ShifterUnit {
    latency: u8,
    state: ShifterState,
}

impl ShifterUnit {
    pub(super) fn new(latency: u8) -> Self {
        assert!(latency >= 1, "쉬프터의 지연시간은 1 이상이어야 합니다.");

        Self {
            latency,
            state: ShifterState::Idle,
        }
    }

    pub(super) fn state(&self) -> ShifterState {
        self.state
    }

    /// 상태와 시간을 변경하지 않는 시프트 계산.
    fn calculate_result(request: AluRequest) -> AluResult {
        let AluRequest { operation, a, b } = request;

        // RV32 시프트 연산은 시프트 양의 하위 5비트를 사용한다.
        let shamt = b & 0x1F;

        let value = match operation {
            AluOperation::Sll => a << shamt,
            AluOperation::Srl => a >> shamt,
            AluOperation::Sra => ((a as i32) >> shamt) as u32,

            _ => unreachable!("쉬프터에 지원하지 않는 연산이 전달되었습니다."),
        };

        AluResult::new(value)
    }

    /// 완료 결과의 전달 및 대기를 처리한다.
    fn finish(
        result: AluResult,
        result_ready: bool,
        request_ready: bool,
    ) -> (AluOutputs, ShifterState) {
        let next = if result_ready {
            ShifterState::Idle
        } else {
            ShifterState::ResultReady { result }
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

impl Clocked for ShifterUnit {
    type Inputs = AluInputs;
    type Outputs = AluOutputs;
    type NextState = ShifterState;

    fn evaluate(&self, inputs: &Self::Inputs) -> (Self::Outputs, Self::NextState) {
        // 취소 사이클에는 요청과 결과 전달을 모두 차단한다.
        if inputs.flush {
            return (AluOutputs::default(), ShifterState::Idle);
        }

        match self.state {
            ShifterState::Idle => {
                let Some(request) = inputs.request else {
                    return (
                        AluOutputs {
                            request_ready: true,
                            result: None,
                        },
                        ShifterState::Idle,
                    );
                };

                assert_eq!(
                    request.operation.unit(),
                    UnitKind::Shift,
                    "쉬프터로 잘못된 요청이 전달되었습니다."
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
                        ShifterState::Running {
                            cycles_left: self.latency - 1,
                            request,
                        },
                    )
                }
            }

            ShifterState::Running {
                cycles_left,
                request,
            } => {
                if cycles_left > 1 {
                    (
                        AluOutputs::default(),
                        ShifterState::Running {
                            cycles_left: cycles_left - 1,
                            request,
                        },
                    )
                } else {
                    // 현재 입력이 아니라 수락 시 저장한 요청을 사용한다.
                    let result = Self::calculate_result(request);

                    Self::finish(result, inputs.result_ready, false)
                }
            }

            ShifterState::ResultReady { result } => {
                // 수락 전까지 기존 결과를 그대로 유지한다.
                Self::finish(result, inputs.result_ready, false)
            }
        }
    }

    fn commit(&mut self, next: Self::NextState) {
        self.state = next;
    }
}
