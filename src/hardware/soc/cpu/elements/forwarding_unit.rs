use crate::hardware::soc::cpu::pipeline_stage::{IdExRegister, MemWbRegister};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardA {
    NoForward = 0,      // EX 단계에서 rs1에 대한 forwarding이 필요 없음
    ForwardFromMem = 1, // MEM 단계에서 rs1에 대한 forwarding 필요 (1 cycle 전)
    ForwardFromWb = 2,  // WB 단계에서 rs1에 대한 forwarding 필요 (2 cycle 전)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardB {
    NoForward = 0,      // EX 단계에서 rs2에 대한 forwarding이 필요 없음
    ForwardFromMem = 1, // MEM 단계에서 rs2에 대한 forwarding 필요 (1 cycle 전)
    ForwardFromWb = 2,  // WB 단계에서 rs2에 대한 forwarding 필요 (2 cycle 전)
}

// forwarding unit 구현
pub struct ForwardingUnit;

impl ForwardingUnit {
    pub fn get_forward_signals(
        id_ex_reg: &IdExRegister,
        mem_reg: &MemWbRegister,
        wb_reg: &MemWbRegister,
    ) -> (ForwardA, ForwardB) {
        let mut forward_a = ForwardA::NoForward;
        let mut forward_b = ForwardB::NoForward;

        // MEM Stage Forwarding
        if mem_reg.control.reg_write && (mem_reg.rd != 0) && (mem_reg.rd == id_ex_reg.rs1) {
            forward_a = ForwardA::ForwardFromMem;
        }
        if mem_reg.control.reg_write && (mem_reg.rd != 0) && (mem_reg.rd == id_ex_reg.rs2) {
            forward_b = ForwardB::ForwardFromMem;
        }

        // WB forwarding has lower priority than MEM forwarding.
        if forward_a == ForwardA::NoForward
            && wb_reg.control.reg_write
            && (wb_reg.rd != 0)
            && (wb_reg.rd == id_ex_reg.rs1)
        {
            forward_a = ForwardA::ForwardFromWb;
        }
        if forward_b == ForwardB::NoForward
            && wb_reg.control.reg_write
            && (wb_reg.rd != 0)
            && (wb_reg.rd == id_ex_reg.rs2)
        {
            forward_b = ForwardB::ForwardFromWb;
        }

        (forward_a, forward_b)
    }
}
