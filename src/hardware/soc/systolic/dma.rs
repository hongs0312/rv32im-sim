/*
    Systolic Array DMA State Machine
    DMA = Direct Memory Access

    시스템 메모리로부터 Systolic Array(Scratchpad)로 데이터를 로드하는 모듈
*/

use super::scratchpad::Scratchpad;
use super::{ARRAY_SIZE, INNER_DIM};
use crate::hardware::soc::system_bus::{BusOwner, SystemBus};
use crate::hardware::soc::types::OpStatus;

#[derive(Clone, Copy, PartialEq)]
pub enum DmaState {
    Idle,
    Bursting { is_a: bool, row: usize, col: usize },
    StoringC { row: usize, col: usize },
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
        Self {
            state: DmaState::Idle,
            addr_a: 0,
            addr_b: 0,
            addr_c: 0,
        }
    }

    pub fn start_load(&mut self, addr_a: u32, addr_b: u32) {
        self.addr_a = addr_a;
        self.addr_b = addr_b;

        self.state = DmaState::Bursting {
            is_a: true,
            row: 0,
            col: 0,
        };
    }

    pub fn start_store(&mut self, addr_c: u32) {
        self.addr_c = addr_c;
        self.state = DmaState::StoringC { row: 0, col: 0 };
    }

    pub fn store_position(&self) -> Option<(usize, usize)> {
        if let DmaState::StoringC { row, col } = self.state {
            Some((row, col))
        } else {
            None
        }
    }

    pub fn step(&mut self, bus: &mut SystemBus, scratchpad: &mut Scratchpad) {
        match self.state {
            DmaState::Idle | DmaState::Done | DmaState::StoringC { .. } => {}

            DmaState::Bursting { is_a, row, col } => {
                if is_a {
                    self.process_burst_a(bus, scratchpad, row, col);
                } else {
                    self.process_burst_b(bus, scratchpad, row, col);
                }
            }
        }
    }

    pub fn store_step(&mut self, bus: &mut SystemBus, values: [u32; 4]) {
        let DmaState::StoringC { row, col } = self.state else {
            return;
        };
        let offset = (row * ARRAY_SIZE + col) * 4;

        // u32 배열을 16바이트 블록으로 직렬화 (루프로 간소화)
        let mut block = [0u8; 16];
        for (i, val) in values.iter().enumerate() {
            block[i * 4..(i + 1) * 4].copy_from_slice(&val.to_le_bytes());
        }

        if matches!(
            bus.write_block(BusOwner::SystolicDma, self.addr_c + offset as u32, &block),
            OpStatus::Complete(())
        ) {
            self.advance_store_state(row, col);
        }
    }

    // --- 행렬 A 처리 ---
    fn process_burst_a(
        &mut self,
        bus: &mut SystemBus,
        scratchpad: &mut Scratchpad,
        row: usize,
        col: usize,
    ) {
        let offset = (row * INNER_DIM + col) * 4;

        if let OpStatus::Complete(block) =
            bus.read_block(BusOwner::SystolicDma, self.addr_a + offset as u32)
        {
            for (index, bytes) in block.chunks_exact(4).enumerate() {
                let current_col = col + index;
                if current_col < INNER_DIM {
                    scratchpad.write_a(
                        row,
                        current_col,
                        u32::from_le_bytes(bytes.try_into().unwrap()),
                    );
                }
            }
            self.advance_burst_a_state(row, col);
        }
    }

    fn advance_burst_a_state(&mut self, row: usize, col: usize) {
        let next_col = col + 4;
        if next_col >= INNER_DIM {
            let next_row = row + 1;
            if next_row >= ARRAY_SIZE {
                self.state = DmaState::Bursting {
                    is_a: false,
                    row: 0,
                    col: 0,
                };
            } else {
                self.state = DmaState::Bursting {
                    is_a: true,
                    row: next_row,
                    col: 0,
                };
            }
        } else {
            self.state = DmaState::Bursting {
                is_a: true,
                row,
                col: next_col,
            };
        }
    }

    // --- 행렬 B 처리 ---
    fn process_burst_b(
        &mut self,
        bus: &mut SystemBus,
        scratchpad: &mut Scratchpad,
        row: usize,
        col: usize,
    ) {
        let offset = (row * ARRAY_SIZE + col) * 4;

        if let OpStatus::Complete(block) =
            bus.read_block(BusOwner::SystolicDma, self.addr_b + offset as u32)
        {
            for (index, bytes) in block.chunks_exact(4).enumerate() {
                let current_col = col + index;
                if current_col < ARRAY_SIZE {
                    scratchpad.write_b(
                        current_col,
                        row,
                        u32::from_le_bytes(bytes.try_into().unwrap()),
                    );
                }
            }
            self.advance_burst_b_state(row, col);
        }
    }
    fn advance_burst_b_state(&mut self, row: usize, col: usize) {
        let next_col = col + 4;
        if next_col >= ARRAY_SIZE {
            let next_row = row + 1;
            if next_row >= INNER_DIM {
                self.state = DmaState::Done;
            } else {
                self.state = DmaState::Bursting {
                    is_a: false,
                    row: next_row,
                    col: 0,
                };
            }
        } else {
            self.state = DmaState::Bursting {
                is_a: false,
                row,
                col: next_col,
            };
        }
    }

    // --- 결과 행렬 C 저장 인덱스 갱신 ---
    fn advance_store_state(&mut self, row: usize, col: usize) {
        let next_col = col + 4;
        if next_col >= ARRAY_SIZE {
            let next_row = row + 1;
            if next_row >= ARRAY_SIZE {
                self.state = DmaState::Done;
            } else {
                self.state = DmaState::StoringC {
                    row: next_row,
                    col: 0,
                };
            }
        } else {
            self.state = DmaState::StoringC { row, col: next_col };
        }
    }
}
