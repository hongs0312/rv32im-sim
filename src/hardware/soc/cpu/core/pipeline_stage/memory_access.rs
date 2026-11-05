use super::{ExMemRegister, MemWbRegister};

use crate::hardware::soc::cpu::core::Core;
use crate::hardware::soc::cpu::core::Trap;
use crate::hardware::soc::cpu::core::components::mmu::{AccessType, Mmu};
use crate::hardware::soc::system_bus::{BusOwner, SystemBus};
use crate::hardware::soc::types::OpStatus;

pub fn execute(
    core: &mut Core,
    bus: &mut SystemBus,
    ex_mem_reg: ExMemRegister,
) -> Result<OpStatus<MemWbRegister>, Trap> {
    let control = ex_mem_reg.control;
    let vaddr = ex_mem_reg.alu_result; // ALU 결과를 가상 주소로 사용

    // 특수 제어: D-Cache 플러시 요청 처리
    if control.is_fence {
        match core.d_cache.handle_flush(bus, BusOwner::DCache) {
            OpStatus::Busy => return Ok(OpStatus::Busy),
            OpStatus::Complete(_) => {
                // 💡 플러시가 끝난 직후, 캐시를 완전히 비워서 다음 읽기 때 강제로 DRAM에서 가져오게 함
                core.d_cache.invalidate_all();

                return Ok(OpStatus::Complete(MemWbRegister {
                    control,
                    alu_result: vaddr,
                    mem_data: 0,
                    rd: ex_mem_reg.rd,
                }));
            }
        }
    }

    // 1. 메모리 접근이 없는 명령어 (ADD, SUB, CSR 등)
    if !control.mem_read && !control.mem_write {
        return Ok(OpStatus::Complete(MemWbRegister {
            control,
            alu_result: vaddr,
            mem_data: 0,
            rd: ex_mem_reg.rd,
        }));
    }

    // 2. 가상 주소 -> 물리 주소 변환 (MMU 사용)
    let access_type = match control.mem_write {
        true => AccessType::Store,
        false => AccessType::Load,
    };
    let priv_mode = core.privilege_mode;
    let sstatus = core.csr.mstatus;
    let paddr = Mmu::translate(
        vaddr,
        access_type,
        priv_mode,
        core.csr.satp,
        sstatus,
        |addr| bus.read_paddr_u32_sync(addr),
    )?;

    // 3. MMIO 라우팅 (캐시 우회 - CLINT, Systolic Array 등 버스에 등록된 모든 장치)
    // 가상 주소가 아닌 물리 주소를 사용하여 MMIO 접근
    if bus.is_mmio(paddr) {
        let mem_data = if control.mem_write {
            let _ = bus.write_mmio(paddr, ex_mem_reg.rs2_data);
            0
        } else {
            bus.read_mmio(paddr)
        };

        return Ok(OpStatus::Complete(MemWbRegister {
            control,
            alu_result: vaddr,
            mem_data,
            rd: ex_mem_reg.rd,
        }));
    }

    // 4. 일반 메모리 접근 (D-Cache 및 DRAM 사용)
    let cache_status = match control.mem_read {
        // D-Cache를 통해 메모리 접근
        // 주소를 4바이트 단위로 정렬하여 캐시 접근
        true => core.d_cache.read(BusOwner::DCache, paddr & !0b11, bus),
        false => core.d_cache.write(
            paddr,
            ex_mem_reg.rs2_data,
            control.funct3,
            bus,
            BusOwner::DCache,
        ),
    };

    match cache_status {
        OpStatus::Busy => Ok(OpStatus::Busy),
        OpStatus::Complete(raw_mem_data) => {
            let mem_data = if control.mem_read {
                apply_funct3_mask(paddr, raw_mem_data, control.funct3)
            } else {
                0
            };

            Ok(OpStatus::Complete(MemWbRegister {
                control,
                alu_result: vaddr,
                mem_data,
                rd: ex_mem_reg.rd,
            }))
        }
    }
}

fn apply_funct3_mask(addr: u32, raw_data: u32, funct3: u8) -> u32 {
    let bit_offset = (addr & 0b11) * 8;

    match funct3 {
        0x0 => ((raw_data >> bit_offset) as u8 as i8) as u32, // LB
        0x1 => ((raw_data >> bit_offset) as u16 as i16) as u32, // LH
        0x2 => raw_data,                                      // LW
        0x4 => ((raw_data >> bit_offset) as u8) as u32,       // LBU
        0x5 => ((raw_data >> bit_offset) as u16) as u32,      // LHU
        _ => raw_data,
    }
}
