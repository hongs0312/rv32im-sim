mod cache_line;

const CACHE_LINE_COUNT: usize = 64;

use crate::hardware::soc::{
    system_bus::{BusOwner, SystemBus},
    types::OpStatus,
};
use cache_line::CacheLine;

// 버스가 비동기로 바뀌었으므로 상태도 Issue(요청)와 Wait(대기)로 분리
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheState {
    Idle,
    WriteBackIssue,
    WriteBackWait,
    FetchIssue,
    FetchWait,
    FlushIssue { index: usize },
    FlushWait { index: usize },
}

pub struct L1Cache {
    pub lines: [CacheLine; CACHE_LINE_COUNT],
    pub state: CacheState,
    pub miss_addr: u32, // Miss 발생 시, 요청 주소를 기억
}

impl L1Cache {
    pub fn new() -> Self {
        Self {
            lines: [CacheLine::new(); CACHE_LINE_COUNT],
            state: CacheState::Idle,
            miss_addr: 0,
        }
    }

    pub fn handle_flush(&mut self, bus: &mut SystemBus, owner: BusOwner) -> OpStatus<u32> {
        if self.state == CacheState::Idle {
            self.state = CacheState::FlushIssue { index: 0 };
        }

        loop {
            match self.state {
                CacheState::FlushIssue { index } => {
                    if index >= CACHE_LINE_COUNT {
                        self.state = CacheState::Idle;
                        return OpStatus::Complete(0); // 플러싱 완료!
                    }

                    let line = &self.lines[index];
                    if line.valid && line.dirty {
                        let old_addr = (line.tag << 10) | ((index as u32) << 4);

                        // 1. 쓰기 시도
                        match bus.issue_write(owner, old_addr, &line.data) {
                            OpStatus::Busy => return OpStatus::Busy, // 버스 꽉 참, 다음 사이클에 재시도
                            OpStatus::Complete(()) => {
                                self.state = CacheState::FlushWait { index };
                                return OpStatus::Busy;
                            }
                        }
                    } else {
                        self.lines[index].valid = false;
                        self.state = CacheState::FlushIssue { index: index + 1 };
                    }
                }
                CacheState::FlushWait { index } => {
                    let line = &self.lines[index];
                    let old_addr = (line.tag << 10) | ((index as u32) << 4);

                    // 2. 완료 여부 확인
                    match bus.collect_write(owner, old_addr) {
                        OpStatus::Busy => return OpStatus::Busy, // 뱅크가 아직 기록 중
                        OpStatus::Complete(()) => {
                            self.lines[index].dirty = false;
                            self.lines[index].valid = false; // 무효화

                            // 이 라인 완료! 다음 라인으로 넘어가서 즉시 처리 시도
                            self.state = CacheState::FlushIssue { index: index + 1 };
                        }
                    }
                }
                _ => return OpStatus::Busy,
            }
        }
    }

    pub fn read(&mut self, owner: BusOwner, addr: u32, bus: &mut SystemBus) -> OpStatus<u32> {
        if self.state == CacheState::Idle {
            let offset = (addr & 0xF) as usize;
            let index = ((addr >> 4) & 0x3F) as usize;
            let tag = addr >> 10;
            let line = &self.lines[index];

            if line.valid && line.tag == tag {
                return OpStatus::Complete(line.read32(offset));
            }

            self.miss_addr = addr;
            if line.valid && line.dirty {
                self.state = CacheState::WriteBackIssue;
            } else {
                self.state = CacheState::FetchIssue;
            }
        }

        if let OpStatus::Complete(new_block) = self.handle_miss(owner, bus) {
            let index = ((self.miss_addr >> 4) & 0x3F) as usize;
            let tag = self.miss_addr >> 10;
            let line = &mut self.lines[index];

            line.data = new_block;
            line.valid = true;
            line.tag = tag;
            line.dirty = false;

            self.state = CacheState::Idle;

            // 방금 가져온 데이터가 CPU가 '지금' 요구하는 주소인지 확인합니다.
            // 분기 실패로 PC가 바뀌었다면, 기껏 가져왔어도 CPU에겐 Busy를 줘서 다음 사이클에 새 주소를 요구하게 만듭니다.
            if self.miss_addr == addr {
                let offset = (addr & 0xF) as usize;
                OpStatus::Complete(line.read32(offset))
            } else {
                OpStatus::Busy
            }
        } else {
            OpStatus::Busy
        }
    }

    #[rustfmt::skip]
    pub fn write(&mut self, addr: u32, value: u32, funct3: u8, bus: &mut SystemBus, owner: BusOwner) -> OpStatus<u32> {
        if addr == 0x8000_0030 {
            return self.handle_flush(bus, owner);
        }

        if self.state == CacheState::Idle {
            let offset = (addr & 0xF) as usize;
            let index = ((addr >> 4) & 0x3F) as usize;
            let tag = addr >> 10;
            let line = &self.lines[index];

            if line.valid && line.tag == tag {
                let line_mut = &mut self.lines[index];
                Self::write_to_line(line_mut, offset, value, funct3);
                return OpStatus::Complete(0);
            }

            self.miss_addr = addr;
            if line.valid && line.dirty {
                self.state = CacheState::WriteBackIssue;
            } else {
                self.state = CacheState::FetchIssue;
            }
        }

        if let OpStatus::Complete(new_block) = self.handle_miss(owner, bus) {
            let index = ((self.miss_addr >> 4) & 0x3F) as usize;
            let tag = self.miss_addr >> 10;
            let line = &mut self.lines[index];

            line.data = new_block;
            line.valid = true;
            line.tag = tag;

            self.state = CacheState::Idle;

            if self.miss_addr == addr {
                let offset = (addr & 0xF) as usize;
                Self::write_to_line(line, offset, value, funct3);
                OpStatus::Complete(0)
            } else {
                OpStatus::Busy
            }
        } else {
            OpStatus::Busy
        }
    }

    #[rustfmt::skip]
    fn handle_miss(&mut self, owner: BusOwner, bus: &mut SystemBus) -> OpStatus<[u8; 16]> {
        let index = ((self.miss_addr >> 4) & 0x3F) as usize;
        let line = &self.lines[index];

        match self.state {
            CacheState::WriteBackIssue => {
                let old_addr = (line.tag << 10) | ((index as u32) << 4);
                if let OpStatus::Complete(()) = bus.issue_write(owner, old_addr, &line.data) {
                    self.state = CacheState::WriteBackWait;
                }
                OpStatus::Busy
            }
            CacheState::WriteBackWait => {
                let old_addr = (line.tag << 10) | ((index as u32) << 4);
                if let OpStatus::Complete(()) = bus.collect_write(owner, old_addr) {
                    self.state = CacheState::FetchIssue;
                }
                OpStatus::Busy
            }
            CacheState::FetchIssue => {
                // 💡 CPU가 새 주소를 달라고 떼를 써도, 하던 일(miss_addr) 먼저 끝냅니다!
                if let OpStatus::Complete(()) = bus.issue_read(owner, self.miss_addr) {
                    self.state = CacheState::FetchWait;
                }
                OpStatus::Busy
            }
            CacheState::FetchWait => {
                if let OpStatus::Complete(new_block) = bus.collect_read(owner, self.miss_addr) {
                    OpStatus::Complete(new_block)
                } else {
                    OpStatus::Busy
                }
            }
            _ => OpStatus::Busy,
        }
    }

    fn write_to_line(line: &mut CacheLine, offset: usize, value: u32, funct3: u8) {
        match funct3 {
            0x0 => line.write8(offset, value as u8),
            0x1 => line.write16(offset, value as u16),
            0x2 => line.write32(offset, value),
            _ => panic!("지원하지 않는 Store funct3: {}", funct3),
        }
        line.dirty = true;
    }

    pub fn reset(&mut self) {}
}
