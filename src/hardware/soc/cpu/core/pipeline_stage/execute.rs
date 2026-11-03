use crate::hardware::soc::cpu::core::Core;
use crate::hardware::soc::cpu::core::control;
use crate::hardware::soc::cpu::core::forwarding_unit::{ForwardA, ForwardB, ForwardingUnit};
use crate::hardware::soc::cpu::core::{PrivilegeMode, Trap};
use crate::hardware::soc::types::OpStatus;

use super::{ExMemRegister, IdExRegister, MemWbRegister};

pub fn execute(
    core: &mut Core,
    id_ex_reg: IdExRegister,
    next_mem_wb_reg: &MemWbRegister,
) -> Result<OpStatus<ExMemRegister>, Trap> {
    let (control, pc, rd, imm) = (id_ex_reg.control, id_ex_reg.pc, id_ex_reg.rd, id_ex_reg.imm);
    let (funct3, funct7) = (control.funct3, control.funct7);

    let rs1_data = core.regs.read(id_ex_reg.rs1);
    let rs2_data = core.regs.read(id_ex_reg.rs2);

    // 무조건 점프(JAL) 중 rd=x0 인 경우 무시되는 것을 막기 위해 !control.jump 조건 추가
    if !control.reg_write
        && !control.mem_write
        && !control.branch
        && !control.jump
        && !control.is_ecall
        && !control.is_mret
        && !control.is_sret
        && !control.is_csr
        && !control.is_fence
        && !control.is_fence_i
    {
        return Ok(OpStatus::Complete(ExMemRegister::default()));
    }

    let (forward_a, forward_b) =
        ForwardingUnit::get_forward_signals(&id_ex_reg, next_mem_wb_reg, &core.mem_wb_reg);

    let rs1_data_forwarded = match forward_a {
        ForwardA::NoForward => rs1_data,
        ForwardA::ForwardFromMem => match next_mem_wb_reg.control.wb_src {
            true => panic!("하드웨어 버그: Load-Use Hazard Stall 누락! rs1 포워딩 불가"),
            false => next_mem_wb_reg.alu_result,
        },
        ForwardA::ForwardFromWb => match core.mem_wb_reg.control.wb_src {
            true => core.mem_wb_reg.mem_data, // 여기서는 포워딩 가능 (MEM 단계 완료됨)
            false => core.mem_wb_reg.alu_result,
        },
    };

    let rs2_data_forwarded = match forward_b {
        ForwardB::NoForward => rs2_data,
        ForwardB::ForwardFromMem => match next_mem_wb_reg.control.wb_src {
            true => panic!("하드웨어 버그: Load-Use Hazard Stall 누락! rs2 포워딩 불가"),
            false => next_mem_wb_reg.alu_result,
        },
        ForwardB::ForwardFromWb => match core.mem_wb_reg.control.wb_src {
            true => core.mem_wb_reg.mem_data,
            false => core.mem_wb_reg.alu_result,
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

    let alu_status = core
        .alu
        .execute_with_cycles(a, b, control.alu_op, funct3, funct7);

    let (raw_alu_result, zero) = match alu_status {
        OpStatus::Busy => return Ok(OpStatus::Busy),
        OpStatus::Complete(res) => res,
    };

    let target_pc = match control.is_jalr {
        true => raw_alu_result & !1,
        false => pc.wrapping_add(imm as u32),
    };

    let mut alu_result = match control.jump {
        true => pc.wrapping_add(4),
        false => raw_alu_result,
    };

    // 실행 단계에서 발생하는 모든 동기 예외(Trap) 원인을 하나로 수집
    let mut trap_cause: Option<u32> = None;

    // ecall, csr 명령어 처리
    if control.is_ecall {
        trap_cause = Some(match core.privilege_mode {
            PrivilegeMode::User => 8,       // User Environment Call
            PrivilegeMode::Supervisor => 9, // Supervisor Environment Call
            PrivilegeMode::Machine => 11,   // Machine Environment Call
        });
    } else if control.is_csr {
        let csr_addr = (imm & 0xFFF) as u16;
        let is_csrrw = (control.csr_op & 0b011) == 0b001;
        let is_write_attempt = is_csrrw || (id_ex_reg.rs1 != 0);

        match core.csr.read(csr_addr, core.privilege_mode) {
            Ok(old_csr_val) => {
                let src_val = if (control.csr_op & 0b100) != 0 {
                    id_ex_reg.rs1 as u32
                } else {
                    rs1_data_forwarded
                };

                let new_csr_val = match control.csr_op & 0b011 {
                    0b001 => src_val,
                    0b010 => old_csr_val | src_val,
                    0b011 => old_csr_val & !src_val,
                    _ => old_csr_val,
                };

                if is_write_attempt
                    && core
                        .csr
                        .write(csr_addr, new_csr_val, core.privilege_mode)
                        .is_err()
                {
                    trap_cause = Some(2); // Read-only CSR 쓰기 위반 등
                } else {
                    alu_result = old_csr_val;
                }
            }
            Err(_) => {
                trap_cause = Some(2); // 권한 위반(Privilege violation) 등
            }
        }
    }

    if let Some(cause) = trap_cause {
        let tval = match cause {
            2 => reassamble_instruction(control, id_ex_reg),
            // Load/Store Misaligned(4, 6), Access Fault(5, 7), Page Fault(13, 15)
            4 | 5 | 6 | 7 | 13 | 15 => alu_result,
            _ => {
                // sim_log!(
                //     "DEBUG: Trap 발생! cause: {}, pc: 0x{:08X}\ntarget(mtvec): 0x{:08X}, a0: 0x{:08X}, a1: 0x{:08X}",
                //     cause, pc, core.csr.mtvec, core.regs.read(10), core.regs.read(11)
                // );
                0
            }
        };

        #[rustfmt::skip]
        return Err(Trap { cause, tval });
    }

    let result_reg = ExMemRegister {
        control,
        pc: id_ex_reg.pc,
        target_pc,
        zero,
        alu_result,
        rd,
        rs2_data: rs2_data_forwarded,
    };
    Ok(OpStatus::Complete(result_reg))
}

fn reassamble_instruction(control: control::ControlSignals, id_ex_reg: IdExRegister) -> u32 {
    let opcode = control.opcode as u32;
    let funct3 = control.funct3 as u32;
    let funct7 = control.funct7 as u32;

    let rs1 = id_ex_reg.rs1 as u32;
    let rs2 = id_ex_reg.rs2 as u32;
    let rd = id_ex_reg.rd as u32;

    let imm = id_ex_reg.imm as u32;

    match opcode {
        0x33 => {
            // R-type
            (funct7 << 25) | (rs2 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
        }
        0x13 => {
            // I-type
            (imm << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode
        }
        _ => {
            // 다른 형식의 명령어는 필요에 따라 추가 구현 가능
            0
        }
    }
}
