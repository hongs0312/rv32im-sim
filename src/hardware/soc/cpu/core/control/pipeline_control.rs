use crate::hardware::soc::cpu::core::pipeline_registers::{
    ExMemRegister, IdExRegister, IfIdRegister, MemWbRegister, PipelineState, StageResults,
};

/// 단계별 정지 원인.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct StallSignals {
    pub if_busy: bool,
    pub load_use: bool,
    pub ex_busy: bool,
    pub mem_busy: bool,
}

/// 파이프라인 레지스터에 적용할 제어 신호.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PipelineControl {
    pub stalls: StallSignals,
    pub redirect_pc: Option<u32>,
    pub inject_nop: bool,
}

impl PipelineControl {
    /// 현재 래치와 입력 후보로 다음 래치를 계산한다.
    ///
    /// 현재 상태를 변경하지 않는 조합 제어 함수다.
    pub fn evaluate(&self, current: PipelineState, results: StageResults) -> PipelineState {
        let stall_mem = self.stalls.mem_busy;
        let stall_ex = stall_mem || self.stalls.ex_busy;
        let stall_id = stall_ex || self.stalls.load_use;
        let stall_if = stall_id || self.stalls.if_busy;

        let mut next = current;

        if let Some(target) = self.redirect_pc {
            next.pc = target;
            next.if_id = IfIdRegister::default();
            next.id_ex = IdExRegister::default();
        } else {
            if !stall_if && !self.inject_nop {
                next.pc = current.pc.wrapping_add(4);
            }

            if !stall_id {
                next.if_id = if self.stalls.if_busy {
                    IfIdRegister::default()
                } else {
                    results.if_id
                };
            }

            if !stall_ex {
                next.id_ex = if self.stalls.load_use {
                    IdExRegister::default()
                } else {
                    results.id_ex
                };
            }
        }

        if !stall_mem {
            next.ex_mem = if self.stalls.ex_busy {
                ExMemRegister::default()
            } else {
                results.ex_mem
            };
        }

        next.mem_wb = if stall_mem {
            MemWbRegister::default()
        } else {
            results.mem_wb
        };

        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_stall_holds_front_pipeline() {
        let current = PipelineState {
            pc: 0x8000_0000,
            ..Default::default()
        };

        let control = PipelineControl {
            stalls: StallSignals {
                mem_busy: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let next = control.evaluate(current, StageResults::default());

        assert_eq!(next.pc, current.pc);
        assert_eq!(next.if_id, current.if_id);
        assert_eq!(next.id_ex, current.id_ex);
        assert_eq!(next.ex_mem, current.ex_mem);
        assert_eq!(next.mem_wb, MemWbRegister::default());
    }

    #[test]
    fn load_use_holds_if_id_and_inserts_ex_bubble() {
        let current = PipelineState {
            pc: 0x8000_0000,
            if_id: IfIdRegister {
                pc: 0x8000_0000,
                instruction: 0x0000_0013,
            },
            ..Default::default()
        };

        let control = PipelineControl {
            stalls: StallSignals {
                load_use: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let next = control.evaluate(current, StageResults::default());

        assert_eq!(next.pc, current.pc);
        assert_eq!(next.if_id, current.if_id);
        assert_eq!(next.id_ex, IdExRegister::default());
    }

    #[test]
    fn redirect_flushes_front_pipeline() {
        let current = PipelineState::default();

        let control = PipelineControl {
            redirect_pc: Some(0x8000_0100),
            ..Default::default()
        };

        let next = control.evaluate(current, StageResults::default());

        assert_eq!(next.pc, 0x8000_0100);
        assert_eq!(next.if_id, IfIdRegister::default());
        assert_eq!(next.id_ex, IdExRegister::default());
    }
}
