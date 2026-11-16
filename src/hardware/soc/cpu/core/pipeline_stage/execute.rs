use crate::hardware::clocked::Clocked;
use crate::hardware::soc::cpu::core::Core;
use crate::hardware::soc::cpu::core::components::alu::{
    AluInputs, AluNextState, AluOperation, AluRequest,
};
use crate::hardware::soc::cpu::core::control;
use crate::hardware::soc::cpu::core::control::forwarding_unit::{
    ForwardA, ForwardB, ForwardingUnit,
};
use crate::hardware::soc::cpu::core::{PrivilegeMode, Trap};
use crate::hardware::soc::types::OpStatus;

use super::{ExMemRegister, IdExRegister, MemWbRegister};

pub fn execute(
    core: &mut Core,
    id_ex_reg: IdExRegister,
    next_mem_wb_reg: &MemWbRegister,
    alu_result_ready: bool,
    next_alu_state: &mut Option<AluNextState>,
) -> Result<OpStatus<ExMemRegister>, Trap> {
    let (control, pc, rd, imm) = (id_ex_reg.control, id_ex_reg.pc, id_ex_reg.rd, id_ex_reg.imm);
    let (funct3, funct7) = (control.funct3, control.funct7);

    let rs1_data = core.regs.read(id_ex_reg.rs1);
    let rs2_data = core.regs.read(id_ex_reg.rs2);

    // SYSTEM opcode에서 funct3=100은 예약된 형식이다.
    // 부작용 없는 명령의 조기 반환보다 먼저 검사해야 한다.
    if control.opcode == 0x73 && funct3 == 0b100 {
        // SYSTEM의 I형식 필드로 원래 인코딩을 복원한다.
        // imm_gen()이 SYSTEM의 imm[11:0]을 보존한다는 전제다.
        let instruction = (((imm as u32) & 0xFFF) << 20)
            | (((id_ex_reg.rs1 as u32) & 0x1F) << 15)
            | ((funct3 as u32) << 12)
            | (((rd as u32) & 0x1F) << 7)
            | 0x73;

        return Err(Trap {
            cause: 2,
            tval: instruction,
        });
    }

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

    let operation = if control.is_csr
        || control.is_ecall
        || control.is_mret
        || control.is_sret
        || control.is_fence
        || control.is_fence_i
    {
        AluOperation::Add
    } else {
        AluOperation::decode(control.alu_op, funct3, funct7)
            .map_err(|_| Trap { cause: 2, tval: 0 })?
    };
    // 2. 현재 상태와 입력으로 출력 및 다음 상태를 계산한다.
    let inputs = AluInputs {
        request: Some(AluRequest { operation, a, b }),
        result_ready: alu_result_ready,
        flush: false,
    };

    let (outputs, next_state) = core.alu.evaluate(&inputs);

    // 3. 다음 상태는 저장만 한다.
    // 여기서 core.alu.commit()을 호출하지 않는다.
    *next_alu_state = Some(next_state);

    // 4. 결과가 없으면 EX는 아직 실행 중이다.
    let result = match outputs.result {
        Some(result) => result,
        None => return Ok(OpStatus::Busy),
    };

    let raw_alu_result = result.value;
    let zero = result.zero;

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
