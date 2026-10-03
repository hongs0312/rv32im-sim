use crate::hardware::soc::cpu::Cpu;
use crate::hardware::soc::system_bus::{BusOwner, SystemBus};
use crate::hardware::soc::types::OpStatus;

use super::IfIdRegister;

pub fn execute(cpu: &mut Cpu, bus: &mut SystemBus, inject_nop: bool) -> OpStatus<IfIdRegister> {
    if inject_nop {
        return OpStatus::Complete(IfIdRegister {
            pc: cpu.pc,
            instruction: 0x00000013,
        });
    }

    let cur_pc = cpu.pc;

    if cur_pc >= 0x8000_0000 {
        panic!(
            "[Error] PC가 MMIO 영역(0x{:08X})을 실행하려고 시도했습니다!",
            cur_pc
        );
    }

    match cpu.i_cache.read(BusOwner::ICache, cur_pc, bus) {
        OpStatus::Busy => OpStatus::Busy,
        OpStatus::Complete(instruction) => OpStatus::Complete(IfIdRegister {
            pc: cur_pc,
            instruction,
        }),
    }
}
