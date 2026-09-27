mod cache_line;

use crate::hardware::soc::{
    cpu::StageStatus,
    system_bus::{BusOwner, SystemBus},
};
use cache_line::CacheLine;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    Idle,
    WriteBack,
    Fetch,
}

pub struct L1Cache {
    pub lines: [CacheLine; 64],
    pub state: CacheState,
}

impl L1Cache {
    pub fn new() -> Self {
        Self {
            lines: [CacheLine::new(); 64],
            state: CacheState::Idle,
        }
    }

    pub fn read(&mut self, owner: BusOwner, addr: u32, bus: &mut SystemBus) -> StageStatus<u32> {
        let offset = (addr & 0xF) as usize;
        let index = ((addr >> 4) & 0x3F) as usize;
        let tag = addr >> 10;

        let line = &mut self.lines[index];

        if line.valid && line.tag == tag {
            // [Hit] 1사이클 즉시 반환
            return StageStatus::Complete(line.read32(offset));
        }

        if self.state == CacheState::Idle {
            if line.valid && line.dirty {
                self.state = CacheState::WriteBack;
            } else {
                self.state = CacheState::Fetch;
            }
        }

        if self.state == CacheState::WriteBack {
            let old_addr = (line.tag << 10) | ((index as u32) << 4);

            match bus.write_block(owner, old_addr, &line.data) {
                StageStatus::Busy => return StageStatus::Busy, // 버스가 바쁘면 캐시도 바쁨
                StageStatus::Complete(_) => {
                    self.state = CacheState::Fetch; // 쓰기 완료 후 Fetch 단계로 전환
                }
            }
        }

        if self.state == CacheState::Fetch {
            match bus.read_block(owner, addr) {
                StageStatus::Busy => return StageStatus::Busy, // 버스가 바쁘면 캐시도 바쁨
                StageStatus::Complete(new_block) => {
                    line.data = new_block;
                    line.valid = true;
                    line.tag = tag;
                    line.dirty = false;

                    self.state = CacheState::Idle; // Fetch 완료 후 Idle 상태로 전환
                    return StageStatus::Complete(line.read32(offset));
                }
            }
        }

        StageStatus::Busy // 아직 완료되지 않은 경우
    }

    pub fn write(
        &mut self,
        addr: u32,
        value: u32,
        funct3: u8,
        bus: &mut SystemBus,
        owner: BusOwner,
    ) -> StageStatus<u32> {
        let offset = (addr & 0xF) as usize;
        let index = ((addr >> 4) & 0x3F) as usize;
        let tag = addr >> 10;

        let line = &mut self.lines[index];

        // 1. [Hit] 평시 상태(Idle)이면서 캐시가 히트된 경우
        if self.state == CacheState::Idle && line.valid && line.tag == tag {
            Self::write_to_line(line, offset, value, funct3);
            return StageStatus::Complete(0);
        }

        // 2. [Miss 발생 시점] 현재 상태가 Idle이면 다음 행동을 결정
        if self.state == CacheState::Idle {
            if line.valid && line.dirty {
                self.state = CacheState::WriteBack; // 방 빼기 시작
            } else {
                self.state = CacheState::Fetch; // 뺄 방이 없으면 바로 새 짐 들이기
            }
        }

        // 3. [Eviction 수행] 기존 데이터를 DRAM으로 쫓아냄
        if self.state == CacheState::WriteBack {
            let old_addr = (line.tag << 10) | ((index as u32) << 4);

            match bus.write_block(owner, old_addr, &line.data) {
                StageStatus::Busy => return StageStatus::Busy, // 5사이클 기다림
                StageStatus::Complete(()) => {
                    self.state = CacheState::Fetch; // 쫓아내기 완료! 이제 Fetch 단계로 넘어감
                }
            }
        }

        // 4. [Fetch 수행] 새 데이터를 DRAM에서 가져옴
        if self.state == CacheState::Fetch {
            match bus.read_block(owner, addr) {
                StageStatus::Busy => return StageStatus::Busy, // 5사이클 기다림
                StageStatus::Complete(new_block) => {
                    // 버스에서 데이터를 가져와서 라인 업데이트
                    line.data = new_block;
                    line.valid = true;
                    line.tag = tag;

                    // 캐시에 온전한 16바이트가 있으니 비로소 원하는 바이트만큼만 덮어쓰기
                    Self::write_to_line(line, offset, value, funct3);

                    self.state = CacheState::Idle; // 상태 초기화
                    return StageStatus::Complete(0);
                }
            }
        }

        StageStatus::Busy
    }

    // 캐시 라인 안에 정확한 크기(8, 16, 32비트)만큼만 덮어쓰고 Dirty 비트를 켜는 헬퍼 메서드
    fn write_to_line(line: &mut CacheLine, offset: usize, value: u32, funct3: u8) {
        match funct3 {
            0x0 => line.write8(offset, value as u8),   // sb (Store Byte)
            0x1 => line.write16(offset, value as u16), // sh (Store Halfword)
            0x2 => line.write32(offset, value),        // sw (Store Word)
            _ => panic!("지원하지 않는 Store funct3: {}", funct3),
        }
        line.dirty = true; // 값이 변경되었으므로 반드시 Dirty 마킹!
    }

    pub fn reset(&mut self) {
        if self.state == CacheState::Fetch {
            self.state = CacheState::Idle;
        }
    }
}
