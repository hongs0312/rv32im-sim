use crate::hardware::soc::memory::Dram;
use crate::hardware::soc::systolic::SystolicArray;
use crate::hardware::soc::types::OpStatus;

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
    pub systolic: Option<&'a mut SystolicArray>,
}

impl<'a> SystemBus<'a> {
    pub fn memory(owner: BusOwner, dram: &'a mut Dram) -> Self {
        Self {
            owner,
            dram,
            systolic: None,
        }
    }

    pub fn with_systolic(
        owner: BusOwner,
        dram: &'a mut Dram,
        systolic: &'a mut SystolicArray,
    ) -> Self {
        Self {
            owner,
            dram,
            systolic: Some(systolic),
        }
    }

    pub fn issue_read(&mut self, owner: BusOwner, addr: u32) -> OpStatus<()> {
        self.owner = owner;

        let base_addr = addr & !0xF;
        match self.dram.issue_read(base_addr) {
            OpStatus::Busy => OpStatus::Busy,
            OpStatus::Complete(()) => {
                self.owner = BusOwner::None;
                OpStatus::Complete(())
            }
        }
    }

    pub fn collect_read(&mut self, owner: BusOwner, addr: u32) -> OpStatus<[u8; 16]> {
        self.owner = owner;

        let base_addr = addr & !0xF;
        match self.dram.collect_read(base_addr) {
            OpStatus::Busy => OpStatus::Busy,
            OpStatus::Complete(block) => {
                self.owner = BusOwner::None;
                OpStatus::Complete(block)
            }
        }
    }

    pub fn issue_write(&mut self, owner: BusOwner, addr: u32, block: &[u8; 16]) -> OpStatus<()> {
        self.owner = owner;

        let base_addr = addr & !0xF;
        match self.dram.issue_write(base_addr, block) {
            OpStatus::Busy => OpStatus::Busy,
            OpStatus::Complete(()) => {
                self.owner = BusOwner::None;
                OpStatus::Complete(())
            }
        }
    }

    pub fn collect_write(&mut self, owner: BusOwner, addr: u32) -> OpStatus<()> {
        self.owner = owner;

        let base_addr = addr & !0xF;
        match self.dram.collect_write(base_addr) {
            OpStatus::Busy => OpStatus::Busy,
            OpStatus::Complete(()) => {
                self.owner = BusOwner::None;
                OpStatus::Complete(())
            }
        }
    }

    // MMIO 읽기 (Systolic 참조 필요)
    pub fn read_mmio(&self, addr: u32) -> u32 {
        let systolic = self
            .systolic
            .as_deref()
            .expect("MMIO requires systolic device");
        match addr {
            0x8000_0000 => systolic.status,
            0x8000_0020 => systolic.global_time,
            _ => 0, // 정의되지 않은 MMIO 주소
        }
    }

    // MMIO 쓰기 (Systolic 가변 참조 필요)
    pub fn write_mmio(&mut self, addr: u32, value: u32) -> Result<u32, ()> {
        let systolic = self
            .systolic
            .as_deref_mut()
            .expect("MMIO requires systolic device");

        match addr {
            0x8000_0004 => systolic.dma.addr_a = value,
            0x8000_0008 => systolic.dma.addr_b = value,
            0x8000_000C => systolic.addr_c = value,
            0x8000_0010 => {
                if value == 1 {
                    // 시작 트리거!
                    systolic.start(systolic.dma.addr_a, systolic.dma.addr_b, systolic.addr_c);
                }
            }
            _ => return Err(()),
        }
        Ok(0)
    }

    pub fn reset(&mut self) {
        self.owner = BusOwner::None;
    }
}

#[cfg(test)]
mod tests {
    use super::{BusOwner, SystemBus};
    use crate::hardware::soc::memory::Dram;
    use crate::hardware::soc::types::OpStatus;

    #[test]
    fn split_transaction_allows_multiple_in_flight_requests() {
        let mut dram = Dram::new(64 * 1024);
        let mut bus = SystemBus::memory(BusOwner::None, &mut dram);

        assert!(matches!(
            bus.issue_read(BusOwner::DCache, 0),
            OpStatus::Complete(()) // 즉시 수락됨!
        ));

        assert!(matches!(
            bus.issue_read(BusOwner::ICache, 16),
            OpStatus::Busy
        ));

        bus.reset();
        bus.dram.tick();

        assert!(matches!(
            bus.issue_read(BusOwner::ICache, 16),
            OpStatus::Complete(())
        ));
        bus.reset();
        assert!(matches!(
            bus.issue_read(BusOwner::DCache, 0),
            OpStatus::Busy
        ));
    }
}
