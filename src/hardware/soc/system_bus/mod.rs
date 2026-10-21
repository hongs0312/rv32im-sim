pub mod mmio_device;

use crate::hardware::soc::memory::Dram;
use crate::hardware::soc::types::OpStatus;

use mmio_device::MmioDevice;

pub const MMIO_BASE: u32 = 0x8000_0000;
pub const MMIO_SIZE: u32 = 0x1000; // 4KB

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusOwner {
    None,
    ICache,
    DCache,
    SystolicDma,
}

pub struct SystemBus<'a> {
    pub owner: BusOwner,
    pub dram: &'a mut Dram,
    pub mmio_devices: &'a mut [(u32, u32, &'a mut dyn MmioDevice)],
}

impl<'a> SystemBus<'a> {
    pub fn new(
        owner: BusOwner,
        dram: &'a mut Dram,
        mmio_devices: &'a mut [(u32, u32, &'a mut dyn MmioDevice)],
    ) -> Self {
        Self {
            owner,
            dram,
            mmio_devices,
        }
    }

    pub fn issue_read(&mut self, owner: BusOwner, addr: u32) -> OpStatus<()> {
        self.execute_dram_op(owner, addr, |dram, base| dram.issue_read(base))
    }

    pub fn collect_read(&mut self, owner: BusOwner, addr: u32) -> OpStatus<[u8; 16]> {
        self.execute_dram_op(owner, addr, |dram, base| dram.collect_read(base))
    }

    pub fn issue_write(&mut self, owner: BusOwner, addr: u32, block: &[u8; 16]) -> OpStatus<()> {
        // block은 클로저가 환경(Environment)에서 자연스럽게 캡처합니다.
        self.execute_dram_op(owner, addr, |dram, base| dram.issue_write(base, block))
    }

    pub fn collect_write(&mut self, owner: BusOwner, addr: u32) -> OpStatus<()> {
        self.execute_dram_op(owner, addr, |dram, base| dram.collect_write(base))
    }

    // 공통 로직을 추출하여 DRAM 작업을 수행하는 함수
    // owner: 현재 버스 소유자, addr: DRAM 주소, op: 수행할 작업 클로저
    // T: 작업 결과 타입, F: 클로저 타입
    // F는 &mut Dram과 u32를 받아 OpStatus<T>를 반환하는 클로저 타입
    fn execute_dram_op<T, F>(&mut self, owner: BusOwner, addr: u32, op: F) -> OpStatus<T>
    where
        F: FnOnce(&mut Dram, u32) -> OpStatus<T>,
    {
        self.owner = owner;
        let base_addr = addr & !0xF;

        match op(self.dram, base_addr) {
            OpStatus::Busy => OpStatus::Busy,
            OpStatus::Complete(result) => {
                self.owner = BusOwner::None;
                OpStatus::Complete(result)
            }
        }
    }

    // MMIO 읽기 (Systolic 참조 필요)
    pub fn read_mmio(&self, addr: u32) -> u32 {
        for (base, size, device) in self.mmio_devices.iter() {
            // MMIO 장치에 대한 주소 범위 확인
            if addr >= *base && addr < *base + *size {
                // MMIO 장치에 대한 읽기 요청을 전달, offset만 전달
                return device.read(addr - *base).unwrap_or(0);
            }
        }
        0 // 정의되지 않은 MMIO 주소
    }

    // MMIO 쓰기 (Systolic 가변 참조 필요)
    pub fn write_mmio(&mut self, addr: u32, value: u32) -> Result<u32, String> {
        for (base, size, device) in self.mmio_devices.iter_mut() {
            if addr >= *base && addr < *base + *size {
                device.write(addr - *base, value)?;
                return Ok(0);
            }
        }
        Err(format!("Invalid MMIO write address: 0x{:08X}", addr))
    }

    pub fn reset(&mut self) {
        self.owner = BusOwner::None;
    }
}

#[cfg(test)]
mod tests {
    use super::mmio_device::MmioDevice;
    use super::{BusOwner, SystemBus};
    use crate::hardware::soc::memory::Dram;
    use crate::hardware::soc::types::OpStatus;

    #[test]
    fn split_transaction_allows_multiple_in_flight_requests() {
        let mut dram = Dram::new(64 * 1024);
        let mut empty_mmio: [(u32, u32, &mut dyn MmioDevice); 0] = [];
        let mut bus = SystemBus::new(BusOwner::None, &mut dram, &mut empty_mmio);

        // 1. 주소 0 (Bank 0)에 읽기 요청 -> 즉시 수락
        assert!(matches!(
            bus.issue_read(BusOwner::DCache, 0),
            OpStatus::Complete(())
        ));

        // 2. 주소 16 (Bank 1)에 읽기 요청
        // 💡 예전엔 여기서 막혔지만, 16-Bank 인터리빙 덕분에 동시 수락(Complete)됩니다!
        assert!(matches!(
            bus.issue_read(BusOwner::ICache, 16),
            OpStatus::Complete(())
        ));

        // 3. Bank Conflict 테스트: 이미 연산 중인 Bank 0(주소 0)에 다시 요청하면 Busy가 발생해야 합니다.
        assert!(matches!(
            bus.issue_read(BusOwner::ICache, 0),
            OpStatus::Busy
        ));

        bus.reset();
        bus.dram.tick(); // 1사이클 경과

        // 4. 사이클이 지나도 Bank 0은 여전히 데이터를 가져오는 중이므로 계속 Busy 상태입니다.
        assert!(matches!(
            bus.issue_read(BusOwner::ICache, 0),
            OpStatus::Busy
        ));
    }
}
