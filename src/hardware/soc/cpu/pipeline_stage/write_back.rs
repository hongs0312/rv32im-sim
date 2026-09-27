use super::MemWbRegister;
use crate::hardware::soc::cpu::Cpu;

pub fn execute(cpu: &mut Cpu, mem_wb_reg: MemWbRegister) {
    let (control, alu_result, mem_data, rd) = (
        mem_wb_reg.control,
        mem_wb_reg.alu_result,
        mem_wb_reg.mem_data,
        mem_wb_reg.rd,
    );

    if control.reg_write {
        let write_data = match control.wb_src {
            false => alu_result,
            true => mem_data,
        };
        cpu.regs.write(rd, write_data, true);
    }
}
