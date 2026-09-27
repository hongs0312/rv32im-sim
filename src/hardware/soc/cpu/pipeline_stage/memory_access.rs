use super::{ExMemRegister, MemWbRegister, StageStatus};
use crate::hardware::soc::cpu::Cpu;
use crate::hardware::soc::system_bus::{BusOwner, SystemBus};

pub fn execute(
    cpu: &mut Cpu,
    bus: &mut SystemBus,
    ex_mem_reg: ExMemRegister,
) -> StageStatus<MemWbRegister> {
    let control = ex_mem_reg.control;
    let addr = ex_mem_reg.alu_result;

    // 1. 메모리 접근이 없는 명령어 (ADD, SUB 등)
    if !control.mem_read && !control.mem_write {
        return StageStatus::Complete(MemWbRegister {
            control,
            alu_result: addr,
            mem_data: 0,
            rd: ex_mem_reg.rd,
        });
    }

    // 2. MMIO 라우팅 (캐시 우회)
    if addr >= 0x8000_0000 {
        let mut mem_data = 0;

        if control.mem_write {
            let _ = bus.write_mmio(addr, ex_mem_reg.rs2_data);
        } else if control.mem_read {
            mem_data = bus.read_mmio(addr);
        }

        return StageStatus::Complete(MemWbRegister {
            control,
            alu_result: addr,
            mem_data,
            rd: ex_mem_reg.rd,
        });
    }

    // 3. 일반 메모리 접근 (D-Cache 사용)
    let cache_status = match control.mem_read {
        true => cpu.d_cache.read(BusOwner::DCache, addr, bus),
        false => cpu.d_cache.write(
            addr,
            ex_mem_reg.rs2_data,
            control.funct3,
            bus,
            BusOwner::DCache,
        ),
    };

    match cache_status {
        StageStatus::Busy => StageStatus::Busy,
        StageStatus::Complete(raw_mem_data) => {
            // Load 일 때만 데이터를 마스킹 처리 (Store일 때는 0 반환)
            let mem_data = if control.mem_read {
                apply_funct3_mask(addr, raw_mem_data, control.funct3)
            } else {
                0
            };

            StageStatus::Complete(MemWbRegister {
                control,
                alu_result: addr,
                mem_data,
                rd: ex_mem_reg.rd,
            })
        }
    }
}

// ----------------------------------------------------------------
// [MEM 전용 헬퍼] 메모리에서 읽어온 32비트 워드를 funct3에 맞게 정육
// ----------------------------------------------------------------
fn apply_funct3_mask(addr: u32, raw_data: u32, funct3: u8) -> u32 {
    // 32비트(4바이트) 덩어리 안에서 실제 타겟 바이트의 시작 위치 계산 (0, 8, 16, 24)
    let bit_offset = (addr & 0b11) * 8;

    match funct3 {
        0x0 => {
            // LB (8비트 추출 및 부호 확장)
            let byte = (raw_data >> bit_offset) as u8;
            (byte as i8) as u32
        }
        0x1 => {
            // LH (16비트 추출 및 부호 확장)
            let halfword = (raw_data >> bit_offset) as u16;
            (halfword as i16) as u32
        }
        0x2 => raw_data, // LW (그대로 통과)
        0x4 => {
            // LBU (8비트 추출, 부호 확장 없음)
            let byte = (raw_data >> bit_offset) as u8;
            byte as u32
        }
        0x5 => {
            // LHU (16비트 추출, 부호 확장 없음)
            let halfword = (raw_data >> bit_offset) as u16;
            halfword as u32
        }
        _ => raw_data,
    }
}
