use crate::hardware::soc::cpu::core::Core;
use crate::hardware::soc::cpu::core::components::mmu::Mmu;
use crate::hardware::soc::on_chip_bus::SystemBus;

use crate::hardware::soc::cpu::core::Trap;
use crate::hardware::soc::cpu::core::components::mmu::AccessType;
use crate::hardware::soc::on_chip_bus::BusOwner;
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
        cur_pc,
        AccessType::Instruction,
        core.privilege_mode,
        satp,
        sstatus,
        |addr| bus.read_paddr_u32_sync(addr),
    )?;

    // 장치 영역 여부는 변환된 물리 주소로 판단한다.
    if bus.is_mmio(paddr) {
        return Err(Trap {
            cause: 1,
            // 예외 정보에는 원래의 명령어 가상 주소를 기록한다.
            tval: cur_pc,
        });
    }

    match core.i_cache.read(BusOwner::ICache, paddr, bus) {
        OpStatus::Busy => Ok(OpStatus::Busy),
        OpStatus::Complete(instruction) => Ok(OpStatus::Complete(IfIdRegister {
            pc: cur_pc,
            instruction,
        })),
    }
}
