// src/hardware/soc/system_bus/mod.rs
use crate::hardware::mmio_device::MmioDevice;
use crate::hardware::soc::memory::Dram;
use crate::hardware::soc::types::OpStatus;

use crate::RAM_SIZE;
use crate::hardware::soc::memory::DRAM_BASE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusOwner {
    None,
    ICache,
    DCache,
    SystolicDma,
    VirtioDma,
}

pub struct SystemBus<'a, 'b, 'c> {
    pub owner: BusOwner,
    pub dram: &'a mut Dram,
    pub internal_mmio: &'a mut [(u32, u32, &'b mut dyn MmioDevice)],
    pub external_mmio: &'a mut [(u32, u32, &'c mut dyn MmioDevice)],
}

impl<'a, 'b, 'c> SystemBus<'a, 'b, 'c> {
    pub fn new(
        owner: BusOwner,
        dram: &'a mut Dram,
        internal_mmio: &'a mut [(u32, u32, &'b mut dyn MmioDevice)],
        external_mmio: &'a mut [(u32, u32, &'c mut dyn MmioDevice)],
    ) -> Self {
        Self {
            owner,
            dram,
            internal_mmio,
            external_mmio,
        }
    }

    // 아래 메서드들(issue_read, is_mmio 등)은 코드 수정 없이 그대로 두시면 됩니다.
    pub fn issue_read(&mut self, owner: BusOwner, addr: u32) -> OpStatus<()> {
        self.execute_dram_op(owner, addr, |dram, base| dram.issue_read(base))
    }

    pub fn collect_read(&mut self, owner: BusOwner, addr: u32) -> OpStatus<[u8; 16]> {
        self.execute_dram_op(owner, addr, |dram, base| dram.collect_read(base))
    }

    pub fn issue_write(&mut self, owner: BusOwner, addr: u32, block: &[u8; 16]) -> OpStatus<()> {
        self.execute_dram_op(owner, addr, |dram, base| dram.issue_write(base, block))
    }

    pub fn collect_write(&mut self, owner: BusOwner, addr: u32) -> OpStatus<()> {
        self.execute_dram_op(owner, addr, |dram, base| dram.collect_write(base))
    }

    fn execute_dram_op<T, F>(&mut self, owner: BusOwner, addr: u32, op: F) -> OpStatus<T>
    where
        F: FnOnce(&mut Dram, u32) -> OpStatus<T>,
    {
        if addr < DRAM_BASE || addr >= DRAM_BASE + (RAM_SIZE as u32) {
            panic!(
                "\n[SystemBus Exception] DRAM 영역 이탈 (0x{:08X})!\n\
                 -> 스택 오버플로우 또는 C 코드 포인터 에러입니다.",
                addr
            );
        }

        let dram_offset = addr.saturating_sub(DRAM_BASE);
        self.owner = owner;
        let base_addr = dram_offset & !0xF;

        match op(self.dram, base_addr) {
            OpStatus::Busy => OpStatus::Busy,
            OpStatus::Complete(result) => {
                self.owner = BusOwner::None;
                OpStatus::Complete(result)
            }
        }
    }

    // 기능 검증용 백도어
    pub fn write_paddr_sync(&mut self, addr: u32, data: &[u8]) {
        if self.is_mmio(addr) {
            // (필요 시 MMIO 동기 쓰기 처리)
        } else if addr >= DRAM_BASE {
            self.dram.load_firmware(addr, data);
        }
    }

    pub fn write_paddr_u32_sync(&mut self, addr: u32, val: u32) {
        self.write_paddr_sync(addr, &val.to_le_bytes());
    }

    pub fn read_paddr_u32_sync(&mut self, addr: u32) -> Result<u32, String> {
        if self.is_mmio(addr) {
            Ok(self.read_mmio(addr))
        } else if addr >= DRAM_BASE && addr < DRAM_BASE + (RAM_SIZE as u32) {
            let offset = addr.saturating_sub(DRAM_BASE);
            Ok(self.dram.load32(offset))
        } else {
            Err(format!("Invalid DRAM read address: 0x{:08X}", addr))
        }
    }

    #[inline]
    fn in_range(addr: u32, base: u32, size: u32) -> bool {
        addr >= base && addr < base.wrapping_add(size)
    }

    pub fn is_mmio(&self, addr: u32) -> bool {
        // 내부 먼저 검사하고 없으면 외부 검사
        self.internal_mmio
            .iter()
            .any(|(base, size, _)| Self::in_range(addr, *base, *size))
            || self
                .external_mmio
                .iter()
                .any(|(base, size, _)| Self::in_range(addr, *base, *size))
    }

    pub fn read_mmio(&mut self, addr: u32) -> u32 {
        // 1. 내부 MMIO에서 먼저 찾기
        if let Some((base, _, device)) = self
            .internal_mmio
            .iter_mut()
            .find(|(base, size, _)| Self::in_range(addr, *base, *size))
        {
            return device.read(addr - *base).unwrap_or(0);
        }

        // 2. 없으면 외부 MMIO에서 찾기
        if let Some((base, _, device)) = self
            .external_mmio
            .iter_mut()
            .find(|(base, size, _)| Self::in_range(addr, *base, *size))
        {
            return device.read(addr - *base).unwrap_or(0);
        }

        0
    }

    pub fn write_mmio(&mut self, addr: u32, value: u32) -> Result<u32, String> {
        // 1. 내부 MMIO에 쓰기 시도
        if let Some((base, _, device)) = self
            .internal_mmio
            .iter_mut()
            .find(|(base, size, _)| Self::in_range(addr, *base, *size))
        {
            device.write(addr - *base, value)?;
            return Ok(0);
        }

        // 2. 외부 MMIO에 쓰기 시도
        if let Some((base, _, device)) = self
            .external_mmio
            .iter_mut()
            .find(|(base, size, _)| Self::in_range(addr, *base, *size))
        {
            device.write(addr - *base, value)?;
            return Ok(0);
        }

        Err(format!("Invalid MMIO write address: 0x{:08X}", addr))
    }

    pub fn reset(&mut self) {
        self.owner = BusOwner::None;
    }
}
