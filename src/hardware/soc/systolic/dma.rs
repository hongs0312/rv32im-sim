/*
    Systolic Array DMA State Machine
    (추후 System DMA로 격상하기 위해 Split-Transaction 완벽 지원)
*/

use super::scratchpad::Scratchpad;
use super::{ARRAY_SIZE, INNER_DIM};
use crate::hardware::soc::system_bus::{BusOwner, SystemBus};
use crate::hardware::soc::types::OpStatus;

#[rustfmt::skip]
#[derive(Clone, Copy, PartialEq)]
pub enum DmaState {
    Idle,
    LoadingA { issue_r: usize, issue_c: usize, collect_r: usize, collect_c: usize },
    LoadingB { issue_r: usize, issue_c: usize, collect_r: usize, collect_c: usize },
    Storing { issue_r: usize, issue_c: usize, collect_r: usize, collect_c: usize },
    Done,
}

pub struct SystolicDma {
    pub state: DmaState,
    pub addr_a: u32,
    pub addr_b: u32,
    pub addr_c: u32,
}

impl SystolicDma {
    pub fn new() -> Self {
        Self { state: DmaState::Idle, addr_a: 0, addr_b: 0, addr_c: 0 }
    }

    pub fn start_load(&mut self, addr_a: u32, addr_b: u32) {
        self.addr_a = addr_a;
        self.addr_b = addr_b;
        self.state = DmaState::LoadingA { issue_r: 0, issue_c: 0, collect_r: 0, collect_c: 0 };
    }

    pub fn start_store(&mut self, addr_c: u32) {
        self.addr_c = addr_c;
        self.state = DmaState::Storing { issue_r: 0, issue_c: 0, collect_r: 0, collect_c: 0 };
    }

    pub fn store_position(&self) -> Option<(usize, usize)> {
        // 💡 시스톨릭 어레이에게 "이번엔 이 주소에 쏠 데이터를 내놔라"고 발사 포인터를 알려줌
        if let DmaState::Storing { issue_r, issue_c, .. } = self.state {
            if issue_r < ARRAY_SIZE {
                Some((issue_r, issue_c))
            } else {
                Some((0, 0))
            }
        } else {
            None
        }
    }

    // 💡 [추가된 헬퍼 함수] 4바이트씩 전진하며 경계를 넘으면 다음 행으로 넘기는 공통 로직
    fn next_ptr(row: usize, col: usize, boundary: usize) -> (usize, usize) {
        let next_c = col + 4;
        if next_c >= boundary { (row + 1, 0) } else { (row, next_c) }
    }

    pub fn step(&mut self, bus: &mut SystemBus, scratchpad: &mut Scratchpad) {
        match self.state {
            DmaState::LoadingA { issue_r, issue_c, collect_r, collect_c } => {
                self.process_pipeline(bus, scratchpad, true, issue_r, issue_c, collect_r, collect_c);
            }
            DmaState::LoadingB { issue_r, issue_c, collect_r, collect_c } => {
                self.process_pipeline(bus, scratchpad, false, issue_r, issue_c, collect_r, collect_c);
            }
            _ => {}
        }
    }

    fn process_pipeline(
        &mut self,
        bus: &mut SystemBus,
        scratchpad: &mut Scratchpad,
        is_a: bool,
        issue_r: usize, issue_c: usize,
        collect_r: usize, collect_c: usize,
    ) {
        // 행렬 A와 B에 따른 설정값 단일화
        let (boundary, end_row, base_addr) = if is_a {
            (INNER_DIM, ARRAY_SIZE, self.addr_a)
        } else {
            (ARRAY_SIZE, INNER_DIM, self.addr_b)
        };

        let mut next_issue = (issue_r, issue_c);
        let mut next_collect = (collect_r, collect_c);

        // 1. [Data Channel] 수거
        if collect_r < end_row {
            let addr = base_addr + ((collect_r * boundary + collect_c) * 4) as u32;
            if let OpStatus::Complete(block) = bus.collect_read(BusOwner::SystolicDma, addr) {
                for (index, bytes) in block.chunks_exact(4).enumerate() {
                    let current_col = collect_c + index;
                    if current_col < boundary {
                        let val = u32::from_le_bytes(bytes.try_into().unwrap());
                        if is_a { scratchpad.write_a(collect_r, current_col, val); }
                        else { scratchpad.write_b(current_col, collect_r, val); }
                    }
                }
                next_collect = Self::next_ptr(collect_r, collect_c, boundary);
            }
        }

        // 2. [Address Channel] 발사
        if issue_r < end_row {
            let addr = base_addr + ((issue_r * boundary + issue_c) * 4) as u32;
            if let OpStatus::Complete(()) = bus.issue_read(BusOwner::SystolicDma, addr) {
                next_issue = Self::next_ptr(issue_r, issue_c, boundary);
            }
        }

        // 3. 상태 전환 (헬퍼 함수 덕분에 재구축 로직이 매우 간결해짐)
        if next_collect.0 >= end_row {
            self.state = if is_a {
                DmaState::LoadingB { issue_r: 0, issue_c: 0, collect_r: 0, collect_c: 0 }
            } else {
                DmaState::Done
            };
        } else {
            self.state = if is_a {
                DmaState::LoadingA { issue_r: next_issue.0, issue_c: next_issue.1, collect_r: next_collect.0, collect_c: next_collect.1 }
            } else {
                DmaState::LoadingB { issue_r: next_issue.0, issue_c: next_issue.1, collect_r: next_collect.0, collect_c: next_collect.1 }
            };
        }
    }

    #[rustfmt::skip]
    pub fn store_step(&mut self, bus: &mut SystemBus, values: [u32; 4]) {
        if let DmaState::Storing { mut issue_r, mut issue_c, mut collect_r, mut collect_c } = self.state {
            
            // 1. [Data Channel] 이전에 쏜 쓰기 요청이 메모리 뱅크에서 끝났는지 확인 (추수)
            if collect_r < ARRAY_SIZE {
                let offset = (collect_r * ARRAY_SIZE + collect_c) * 4;
                if let OpStatus::Complete(()) = bus.collect_write(BusOwner::SystolicDma, self.addr_c + offset as u32) {
                    let (nr, nc) = Self::next_ptr(collect_r, collect_c, ARRAY_SIZE);
                    collect_r = nr;
                    collect_c = nc;
                }
            }

            // 2. [Address Channel] 뱅크가 터지든 말든 쉴 새 없이 새 데이터 발사
            if issue_r < ARRAY_SIZE {
                let offset = (issue_r * ARRAY_SIZE + issue_c) * 4;
                let mut block = [0u8; 16];
                
                for (i, val) in values.iter().enumerate() {
                    block[i * 4..(i + 1) * 4].copy_from_slice(&val.to_le_bytes());
                }

                if let OpStatus::Complete(()) = bus.issue_write(BusOwner::SystolicDma, self.addr_c + offset as u32, &block) {
                    let (nr, nc) = Self::next_ptr(issue_r, issue_c, ARRAY_SIZE);
                    issue_r = nr;
                    issue_c = nc;
                }
            }

            // 3. 상태 업데이트: 추수(collect)가 끝까지 도달해야 진짜 완료!
            if collect_r >= ARRAY_SIZE {
                self.state = DmaState::Done;
            } else {
                self.state = DmaState::Storing { issue_r, issue_c, collect_r, collect_c };
            }
        }
    }
}