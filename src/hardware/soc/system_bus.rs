use crate::hardware::soc::cpu::pipeline_stage::StageStatus;
use crate::hardware::soc::memory::Dram;
use crate::hardware::soc::systolic::SystolicArray;

#[derive(Clone, Copy, PartialEq)]
pub enum BusState {
    Ready,
    Processing(u32), // 남은 대기 사이클
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusOwner {
    None,
    ICache,
    DCache,
    SystolicDma,
}

pub struct SystemBus<'a> {
    pub state: BusState,
    pub owner: BusOwner,
    pub dram: &'a mut Dram,
    pub systolic: Option<&'a mut SystolicArray>,
}

impl<'a> SystemBus<'a> {
    pub fn memory(state: BusState, owner: BusOwner, dram: &'a mut Dram) -> Self {
        Self {
            state,
            owner,
            dram,
            systolic: None,
        }
    }

    pub fn with_systolic(
        state: BusState,
        owner: BusOwner,
        dram: &'a mut Dram,
        systolic: &'a mut SystolicArray,
    ) -> Self {
        Self {
            state,
            owner,
            dram,
            systolic: Some(systolic),
        }
    }

    pub fn read_block(&mut self, owner: BusOwner, addr: u32) -> StageStatus<[u8; 16]> {
        let base_addr = (addr & !0xF) as usize;

        match self.state {
            BusState::Ready => {
                self.owner = owner;
                self.state = BusState::Processing(4); // 5사이클 중 첫 사이클 소모
                StageStatus::Busy
            }
            BusState::Processing(cycles_left) => {
                if self.owner != owner {
                    return StageStatus::Busy;
                }

                if cycles_left > 1 {
                    self.state = BusState::Processing(cycles_left - 1);
                    StageStatus::Busy
                } else {
                    self.state = BusState::Ready;
                    self.owner = BusOwner::None;

                    let mut block = [0u8; 16];
                    block.copy_from_slice(&self.dram.dram[base_addr..base_addr + 16]);
                    StageStatus::Complete(block)
                }
            }
        }
    }

    pub fn write_block(&mut self, owner: BusOwner, addr: u32, block: &[u8; 16]) -> StageStatus<()> {
        let base_addr = (addr & !0xF) as usize;

        match self.state {
            BusState::Ready => {
                self.owner = owner;
                self.state = BusState::Processing(4);
                StageStatus::Busy
            }
            BusState::Processing(cycles_left) => {
                if self.owner != owner {
                    return StageStatus::Busy;
                }

                if cycles_left > 1 {
                    self.state = BusState::Processing(cycles_left - 1);
                    StageStatus::Busy
                } else {
                    self.state = BusState::Ready;
                    self.owner = BusOwner::None;
                    self.dram.dram[base_addr..base_addr + 16].copy_from_slice(block);
                    StageStatus::Complete(())
                }
            }
        }
    }

    pub fn read_word(&mut self, owner: BusOwner, addr: u32) -> StageStatus<u32> {
        match self.read_block(owner, addr) {
            StageStatus::Busy => StageStatus::Busy,
            StageStatus::Complete(block) => {
                let offset = (addr & 0xF) as usize;
                StageStatus::Complete(u32::from_le_bytes(
                    block[offset..offset + 4].try_into().unwrap(),
                ))
            }
        }
    }

    pub fn write_word(&mut self, owner: BusOwner, addr: u32, value: u32) -> StageStatus<()> {
        let address = addr as usize;

        match self.state {
            BusState::Ready => {
                self.owner = owner;
                self.state = BusState::Processing(4);
                StageStatus::Busy
            }
            BusState::Processing(cycles_left) => {
                if self.owner != owner {
                    return StageStatus::Busy;
                }

                if cycles_left > 1 {
                    self.state = BusState::Processing(cycles_left - 1);
                    StageStatus::Busy
                } else {
                    self.state = BusState::Ready;
                    self.owner = BusOwner::None;
                    self.dram.dram[address..address + 4].copy_from_slice(&value.to_le_bytes());
                    StageStatus::Complete(())
                }
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
        self.state = BusState::Ready;
        self.owner = BusOwner::None;
    }
}

#[cfg(test)]
mod tests {
    use super::{BusOwner, BusState, SystemBus};
    use crate::hardware::soc::cpu::pipeline_stage::StageStatus;
    use crate::hardware::soc::memory::Dram;

    #[test]
    fn non_owner_cannot_advance_active_transaction() {
        let mut dram = Dram::new(64);
        let mut bus = SystemBus::memory(BusState::Ready, BusOwner::None, &mut dram);

        assert!(matches!(
            bus.read_block(BusOwner::DCache, 0),
            StageStatus::Busy
        ));
        assert_eq!(bus.owner, BusOwner::DCache);
        assert!(matches!(bus.state, BusState::Processing(4)));

        assert!(matches!(
            bus.read_block(BusOwner::ICache, 0),
            StageStatus::Busy
        ));
        assert!(matches!(bus.state, BusState::Processing(4)));

        assert!(matches!(
            bus.read_block(BusOwner::DCache, 0),
            StageStatus::Busy
        ));
        assert!(matches!(bus.state, BusState::Processing(3)));
    }
}
