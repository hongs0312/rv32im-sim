use crate::hardware::soc::cpu::Cpu;
use crate::hardware::soc::cpu::elements::forwarding_unit::{ForwardA, ForwardB, ForwardingUnit};

use super::{ExMemRegister, IdExRegister, MemWbRegister, StageStatus};

pub fn execute(
    cpu: &mut Cpu,
    id_ex_reg: IdExRegister,
    next_mem_wb_reg: &MemWbRegister,
) -> StageStatus<ExMemRegister> {
    let (control, pc, rd, imm) = (id_ex_reg.control, id_ex_reg.pc, id_ex_reg.rd, id_ex_reg.imm);
    let (funct3, funct7) = (control.funct3, control.funct7);

    let rs1_data = cpu.regs.read(id_ex_reg.rs1);
    let rs2_data = cpu.regs.read(id_ex_reg.rs2);

    // 무조건 점프(JAL) 중 rd=x0 인 경우 무시되는 것을 막기 위해 !control.jump 조건 추가
    if !control.reg_write
        && !control.mem_write
        && !control.branch
        && !control.jump
        && !control.is_ecall
    {
        return StageStatus::Complete(ExMemRegister::default());
    }

    let (forward_a, forward_b) =
        ForwardingUnit::get_forward_signals(&id_ex_reg, next_mem_wb_reg, &cpu.mem_wb_reg);

    let rs1_data_forwarded = match forward_a {
        ForwardA::NoForward => rs1_data,
        ForwardA::ForwardFromMem => match next_mem_wb_reg.control.wb_src {
            true => next_mem_wb_reg.mem_data,
            false => next_mem_wb_reg.alu_result,
        },
        ForwardA::ForwardFromWb => match cpu.mem_wb_reg.control.wb_src {
            true => cpu.mem_wb_reg.mem_data,
            false => cpu.mem_wb_reg.alu_result,
        },
    };

    let rs2_data_forwarded = match forward_b {
        ForwardB::NoForward => rs2_data,
        ForwardB::ForwardFromMem => match next_mem_wb_reg.control.wb_src {
            true => next_mem_wb_reg.mem_data,
            false => next_mem_wb_reg.alu_result,
        },
        ForwardB::ForwardFromWb => match cpu.mem_wb_reg.control.wb_src {
            true => cpu.mem_wb_reg.mem_data,
            false => cpu.mem_wb_reg.alu_result,
        },
    };

    let a = match control.alu_src_a {
        true => pc,
        false => rs1_data_forwarded,
    };
    let b = match control.alu_src_b {
        true => imm as u32,
        false => rs2_data_forwarded,
    };

    let alu_status = cpu
        .alu
        .execute_with_cycles(a, b, control.alu_op, funct3, funct7);

    let (raw_alu_result, zero) = match alu_status {
        StageStatus::Busy => return StageStatus::Busy,
        StageStatus::Complete(res) => res,
    };

    let target_pc = match control.is_jalr {
        true => raw_alu_result & !1,
        false => pc.wrapping_add(imm as u32),
    };

    let alu_result = match control.jump {
        true => pc.wrapping_add(4),
        false => raw_alu_result,
    };

    let result_reg = ExMemRegister {
        control,
        target_pc,
        zero,
        alu_result,
        rd,
        rs2_data: rs2_data_forwarded,
    };
    StageStatus::Complete(result_reg)
}
