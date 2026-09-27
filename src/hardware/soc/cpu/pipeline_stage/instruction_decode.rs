use crate::hardware::soc::cpu::Cpu;
use crate::hardware::soc::cpu::elements::{control::control_unit::ControlUnit, decoder::Decoder};

use super::{IdExRegister, IfIdRegister};

pub fn execute(cpu: &mut Cpu, if_id_reg: IfIdRegister) -> IdExRegister {
    let (pc, instruction) = (if_id_reg.pc, if_id_reg.instruction);
    let (funct7, rs2, rs1, funct3, rd, opcode) = Decoder::decode(instruction);

    let rs1_data = cpu.regs.read(rs1);
    let rs2_data = cpu.regs.read(rs2);
    let imm = Decoder::imm_gen(instruction);

    let control = ControlUnit::decode(opcode, funct3, funct7);

    IdExRegister {
        control,
        pc,
        rd,
        rs1,
        rs2,
        rs1_data,
        rs2_data,
        imm,
    }
}
