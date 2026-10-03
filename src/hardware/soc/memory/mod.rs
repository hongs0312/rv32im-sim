use crate::hardware::soc::types::OpStatus;

const DRAN_LATENCY: u8 = 5; // DRAM의 지연 시간 (예: 5 사이클)

#[derive(Clone, Copy, PartialEq)]
pub enum DramState {
    Idle,
    Reading { cycles_left: u8 },
    Writing { cycles_left: u8 },
}

pub struct Dram {
    pub dram: Vec<u8>,
    pub state: DramState,
    pub latency: u8,
}

impl Dram {
    pub fn new(size: usize) -> Self {
        Self {
            dram: vec![0; size],
            state: DramState::Idle,
            latency: DRAN_LATENCY,
        }
    }

    pub fn read_block(&mut self, addr: usize) -> OpStatus<[u8; 16]> {
        match self.state {
            DramState::Idle => {
                // 처음 요청이 들어오면 지연 상태로 진입
                self.state = DramState::Reading {
                    cycles_left: self.latency - 1,
                };
                OpStatus::Busy
            }
            DramState::Reading { cycles_left } => {
                if cycles_left > 1 {
                    // 아직 대기 중
                    self.state = DramState::Reading {
                        cycles_left: cycles_left - 1,
                    };
                    OpStatus::Busy
                } else {
                    // 대기 완료! 데이터 반환 및 Idle 복귀
                    self.state = DramState::Idle;
                    let mut block = [0u8; 16];
                    block.copy_from_slice(&self.dram[addr..addr + 16]);
                    OpStatus::Complete(block)
                }
            }
            _ => OpStatus::Busy, // Write 중인데 Read가 들어온 경우 (충돌)
        }
    }

    pub fn write_block(&mut self, addr: usize, block: &[u8; 16]) -> OpStatus<()> {
        match self.state {
            DramState::Idle => {
                self.state = DramState::Writing {
                    cycles_left: self.latency - 1,
                };
                OpStatus::Busy
            }
            DramState::Writing { cycles_left } => {
                if cycles_left > 1 {
                    self.state = DramState::Writing {
                        cycles_left: cycles_left - 1,
                    };
                    OpStatus::Busy
                } else {
                    self.state = DramState::Idle;
                    self.dram[addr..addr + 16].copy_from_slice(block);
                    OpStatus::Complete(())
                }
            }
            _ => OpStatus::Busy,
        }
    }

    // 캐시구조 도입으로 인해 실제로는 사용하지 않는 메서드지만 디버깅을 위해 남겨둠
    // Load methods
    pub fn load8(&self, addr: usize) -> u8 {
        u8::from_le_bytes([self.dram[addr]])
    }
    pub fn load16(&self, addr: usize) -> u16 {
        let bytes = &self.dram[addr..addr + 2];
        u16::from_le_bytes(bytes.try_into().expect("Slice with incorrect length"))
    }
    pub fn load32(&self, addr: usize) -> u32 {
        let bytes = &self.dram[addr..addr + 4];
        u32::from_le_bytes(bytes.try_into().expect("Slice with incorrect length"))
    }

    // Store methods
    pub fn store8(&mut self, addr: usize, value: u8) {
        self.dram[addr] = value;
    }
    pub fn store16(&mut self, addr: usize, value: u16) {
        let bytes = value.to_le_bytes();
        self.dram[addr..addr + 2].copy_from_slice(&bytes);
    }
    pub fn store32(&mut self, addr: usize, value: u32) {
        let bytes = value.to_le_bytes();
        self.dram[addr..addr + 4].copy_from_slice(&bytes);
    }
}
