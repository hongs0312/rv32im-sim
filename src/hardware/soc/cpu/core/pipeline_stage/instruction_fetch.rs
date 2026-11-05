use crate::hardware::soc::cpu::core::Core;
use crate::hardware::soc::cpu::core::components::mmu::Mmu;
use crate::hardware::soc::system_bus::SystemBus;

use crate::hardware::soc::cpu::core::Trap;
use crate::hardware::soc::cpu::core::components::mmu::AccessType;
use crate::hardware::soc::system_bus::BusOwner;
use crate::hardware::soc::types::OpStatus;

use super::IfIdRegister;

pub fn execute(
    core: &mut Core,
    bus: &mut SystemBus,
    inject_nop: bool,
) -> Result<OpStatus<IfIdRegister>, Trap> {
    if inject_nop {
        return Ok(OpStatus::Complete(IfIdRegister {
            pc: core.pc,
            instruction: 0x00000013,
        }));
    }

    let cur_pc = core.pc;
    let satp = core.csr.satp;
    let sstatus = core.csr.mstatus;

    let paddr = Mmu::translate(
        core.pc,
        AccessType::Instruction,
        core.privilege_mode,
        satp,
        sstatus,
        |addr| bus.read_paddr_u32_sync(addr),
    )?;

    // MMIO 접근 시 예외 발생
    if bus.is_mmio(cur_pc) {
        return Err(Trap {
            cause: 1,
            tval: cur_pc,
        }); // MMIO 접근 시 예외 발생
    }

    match core.i_cache.read(BusOwner::ICache, paddr, bus) {
        OpStatus::Busy => Ok(OpStatus::Busy),
        OpStatus::Complete(instruction) => Ok(OpStatus::Complete(IfIdRegister {
            pc: cur_pc,
            instruction,
        })),
    }
}
