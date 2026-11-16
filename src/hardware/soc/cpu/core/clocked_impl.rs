//! Core의 클럭 경계.
//!
//! evaluate:
//!   현재 상태를 읽고 파이프라인·WB·ALU의 다음 상태를 계산한다.
//!
//! commit:
//!   이미 계산한 변경만 반영한다.
//!
//! 주의:
//!   기존의 가변 단계 함수와 SystemBus를 직접 호출하지 않는다.
//!   단계별 결과와 제어 신호는 CoreInputs로 전달한다.
//!   CSR·캐시의 다음 상태는 후속 전환 대상이다.

use crate::hardware::clocked::Clocked;

use super::components::alu::{AluInputs, AluNextState, AluOutputs, AluRequest};

use super::control::pipeline_control::{PipelineControl, StallSignals};

use super::pipeline_registers::{
    ExMemRegister, IdExRegister, IfIdRegister, MemWbRegister, PipelineState, StageResults,
};

use super::Core;

// ============================================================================
// 레지스터 파일 쓰기 신호
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterWrite {
    pub rd: u8,
    pub value: u32,
}

// ============================================================================
// 단계별 계산 결과
// ============================================================================

/// 각 단계가 생성한 파이프라인 레지스터 입력 후보.
///
/// 이 값을 만드는 과정에서도 현재 상태를 변경하면 안 된다.
/// 기존의 &mut Core 기반 단계 함수를 그대로 호출하는 입력은 아니다.
#[derive(Debug, Clone, Copy, Default)]
pub struct CoreStageSignals {
    pub if_id: IfIdRegister,
    pub id_ex: IdExRegister,
    pub ex_mem: ExMemRegister,
    pub mem_wb: MemWbRegister,
}

/// 각 단계의 정지 원인.
#[derive(Debug, Clone, Copy, Default)]
pub struct CoreStallSignals {
    pub if_busy: bool,
    pub load_use: bool,

    /// ALU 이외의 이유로 EX 처리가 완료되지 않았음.
    ///
    /// ALU 자체의 완료 여부는 Core에서 추가로 반영한다.
    pub ex_busy: bool,

    pub mem_busy: bool,
}

// ============================================================================
// 리다이렉트
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectScope {
    /// 현재 EX 명령은 완료시키고 젊은 IF/ID 명령만 취소한다.
    ///
    /// 정상 분기·점프에 사용한다.
    Frontend,

    /// 현재 EX 명령과 그보다 젊은 명령을 취소한다.
    Execute,

    /// 현재 MEM 명령과 그보다 젊은 명령을 취소한다.
    ///
    /// MEM 단계의 동기 예외 등에 사용한다.
    Memory,
}

/// 명령 순서에 따른 우선순위 판정이 끝난 리다이렉트 후보.
///
/// IF에서 예외를 발견했다는 이유만으로 바로 생성하면 안 된다.
/// 더 오래된 명령과 분기에 의해 취소되는지 먼저 판단해야 한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreRedirect {
    pub target_pc: u32,
    pub scope: RedirectScope,
}

impl CoreRedirect {
    fn discards_ex(self) -> bool {
        matches!(self.scope, RedirectScope::Execute | RedirectScope::Memory)
    }

    fn discards_mem(self) -> bool {
        matches!(self.scope, RedirectScope::Memory)
    }
}

// ============================================================================
// Core 입출력
// ============================================================================

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreInputs {
    pub stages: CoreStageSignals,
    pub stalls: CoreStallSignals,

    /// 현재 EX 명령의 ALU 요청.
    ///
    /// 요청을 수락한 뒤에도 ID/EX 명령 정보는 결과 수락 또는
    /// 취소 시점까지 유지해야 한다.
    ///
    /// 실행 중인 ALU는 None을 전달해도 진행한다.
    pub alu_request: Option<AluRequest>,

    pub redirect: Option<CoreRedirect>,

    /// 기존 제어와 동일하게 PC 증가를 억제한다.
    ///
    /// stages.if_id에는 공급할 NOP 또는 버블 후보를 전달한다.
    pub inject_nop: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct CoreOutputs {
    /// 이번 사이클 WB가 생성한 쓰기 신호.
    pub register_write: Option<RegisterWrite>,

    pub alu: AluOutputs,

    /// 이번 사이클 EX 결과가 EX/MEM으로 전달되는 조건.
    ///
    /// 현재 구조에는 별도 valid가 없으므로,
    /// 실제 명령 존재 여부는 단계 제어가 보장해야 한다.
    pub ex_ready: bool,

    /// 스톨 등을 고려해 실제로 허용된 리다이렉트.
    pub redirect: Option<CoreRedirect>,
}

/// 현재 Clocked 경로에서 반영하는 다음 상태.
///
/// 필드는 외부에서 수정하지 않고 evaluate 결과를
/// 그대로 commit으로 전달한다.
pub struct CoreNextState {
    pipeline: PipelineState,
    register_write: Option<RegisterWrite>,
    alu: AluNextState,
}

// ============================================================================
// 조합 조회
// ============================================================================

impl Core {
    /// WB가 생성하는 쓰기 신호를 계산한다.
    ///
    /// 레지스터 파일은 변경하지 않는다.
    pub fn writeback_signal(&self) -> Option<RegisterWrite> {
        let register = self.mem_wb_reg;

        if !register.control.reg_write || register.rd == 0 {
            return None;
        }

        let value = if register.control.wb_src {
            register.mem_data
        } else {
            register.alu_result
        };

        Some(RegisterWrite {
            rd: register.rd,
            value,
        })
    }

    /// 이번 WB 쓰기를 우회하는 레지스터 읽기.
    ///
    /// 기존의 "WB를 먼저 실행하고 ID/EX가 읽는다"는
    /// 호출 순서 의존성을 명시적인 신호로 대체한다.
    ///
    /// MEM 포워딩이 필요하면 이 값보다 높은 우선순위로 선택한다.
    pub fn read_register_with_writeback(&self, rs: u8) -> u32 {
        if rs == 0 {
            return 0;
        }

        if let Some(write) = self.writeback_signal() {
            if write.rd == rs {
                return write.value;
            }
        }

        self.regs.read(rs)
    }

    /// 단계별 조합 계산에서 사용할 ALU 출력 조회.
    ///
    /// 현재 상태를 변경하지 않고 결과를 소비하지도 않는다.
    /// 여기서 반환한 다음 상태는 사용하지 않는다.
    ///
    /// EX/MEM 입력 후보를 만들 때 이 결과를 사용할 수 있다.
    pub fn alu_signals(&self, request: Option<AluRequest>) -> AluOutputs {
        let (outputs, _) = self.alu.evaluate(&AluInputs {
            request,
            result_ready: false,
            flush: false,
        });

        outputs
    }

    fn accepted_redirect(inputs: &CoreInputs, ex_busy: bool) -> Option<CoreRedirect> {
        let redirect = inputs.redirect?;

        let accepted = match redirect.scope {
            // 자신의 EX 결과를 전달할 수 있어야 분기를 확정한다.
            RedirectScope::Frontend => !inputs.stalls.mem_busy && !ex_busy,

            // 더 오래된 MEM 명령이 대기 중이면 보류한다.
            RedirectScope::Execute => !inputs.stalls.mem_busy,

            // MEM 예외는 해당 명령과 젊은 명령을 취소한다.
            RedirectScope::Memory => true,
        };

        if accepted { Some(redirect) } else { None }
    }
}

// ============================================================================
// Clocked
// ============================================================================

impl Clocked for Core {
    type Inputs = CoreInputs;
    type Outputs = CoreOutputs;
    type NextState = CoreNextState;

    fn evaluate(&self, inputs: &Self::Inputs) -> (Self::Outputs, Self::NextState) {
        let current = self.pipeline_state();
        let register_write = self.writeback_signal();

        // 1. ALU 결과의 준비 여부를 조회한다.
        let alu_preview = self.alu_signals(inputs.alu_request);

        let uses_alu = inputs.alu_request.is_some() || !self.alu.is_idle();

        let alu_busy = uses_alu && alu_preview.result.is_none();
        let ex_busy = inputs.stalls.ex_busy || alu_busy;

        // 2. 리다이렉트와 취소 범위를 결정한다.
        let redirect = Self::accepted_redirect(inputs, ex_busy);

        let discard_ex = redirect.map_or(false, |value| value.discards_ex());

        let discard_mem = redirect.map_or(false, |value| value.discards_mem());

        // 3. EX/MEM의 실제 수락 조건을 결정한다.
        let ex_ready = !inputs.stalls.mem_busy && !ex_busy && !discard_ex;

        // 4. 수락·취소 신호를 반영한 ALU 다음 상태를 계산한다.
        //
        // 앞선 조회와 동일한 현재 상태에서 계산한다.
        // evaluate를 여러 번 호출해도 현재 상태는 바뀌지 않는다.
        // 아래 next_alu만 한 번 commit한다.
        let (alu_outputs, next_alu) = self.alu.evaluate(&AluInputs {
            request: if discard_ex { None } else { inputs.alu_request },

            result_ready: uses_alu && ex_ready,
            flush: discard_ex,
        });

        // 5. 다음 파이프라인 레지스터 값을 계산한다.
        let control = PipelineControl {
            stalls: StallSignals {
                if_busy: inputs.stalls.if_busy,
                load_use: inputs.stalls.load_use,
                ex_busy,
                mem_busy: inputs.stalls.mem_busy,
            },

            redirect_pc: redirect.map(|value| value.target_pc),
            inject_nop: inputs.inject_nop,
        };

        let mut next_pipeline = control.evaluate(
            current,
            StageResults {
                if_id: inputs.stages.if_id,
                id_ex: inputs.stages.id_ex,
                ex_mem: inputs.stages.ex_mem,
                mem_wb: inputs.stages.mem_wb,
            },
        );

        // 6. 예외 등이 취소하는 범위까지 정리한다.
        //
        // 정상 분기·점프는 Frontend이므로 자신의 EX 결과는 보존한다.
        if discard_ex {
            next_pipeline.ex_mem = ExMemRegister::default();
        }

        if discard_mem {
            next_pipeline.mem_wb = MemWbRegister::default();
        }

        let outputs = CoreOutputs {
            register_write,
            alu: alu_outputs,
            ex_ready,
            redirect,
        };

        let next = CoreNextState {
            pipeline: next_pipeline,
            register_write,
            alu: next_alu,
        };

        (outputs, next)
    }

    fn commit(&mut self, next: Self::NextState) {
        // 새로운 판단이나 외부 부품 호출을 하지 않는다.
        // evaluate가 결정한 변경만 반영한다.

        if let Some(write) = next.register_write {
            self.regs.write(write.rd, write.value, true);
        }

        self.alu.commit(next.alu);
        self.commit_pipeline(next.pipeline);
    }
}
