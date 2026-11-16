pub mod dma;
pub mod mmio;
pub mod processing_element;
pub mod scratchpad;

use crate::hardware::soc::on_chip_bus::SystemBus;

use dma::{DmaState, SystolicDma};
use processing_element::ProcessingElement;
use scratchpad::Scratchpad;

use processing_element::MAC_LATENCY;

pub const ARRAY_SIZE: usize = 16;
pub const INNER_DIM: usize = 16;

pub const SYSTOLIC_BASE: u32 = 0x4000_0000;
pub const SYSTOLIC_SIZE: u32 = 0x1000; // 4KB

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct StreamValue {
    pub value: u32,
    pub valid: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SystolicState {
    Idle,
    Loading,
    Computing,
    Storing,
    Done,
}

pub struct SystolicArray {
    pub status: u32,
    pub addr_c: u32,
    pub state: SystolicState,

    pub dma: SystolicDma,
    pub scratchpad: Scratchpad,
    pub pes: [[ProcessingElement; ARRAY_SIZE]; ARRAY_SIZE],

    pub cycle: usize,
    pub global_time: u32,
}

impl SystolicArray {
    pub fn new() -> Self {
        const INIT_PE: ProcessingElement = ProcessingElement::new();

        Self {
            status: 0,
            addr_c: 0,
            state: SystolicState::Idle,

            dma: SystolicDma::new(),
            scratchpad: Scratchpad::new(),
            pes: [[INIT_PE; ARRAY_SIZE]; ARRAY_SIZE],

            cycle: 0,
            global_time: 0,
        }
    }

    pub fn start(&mut self, addr_a: u32, addr_b: u32, addr_c: u32) {
        self.status = 1;
        self.addr_c = addr_c;

        self.scratchpad.clear();

        for r in 0..ARRAY_SIZE {
            for c in 0..ARRAY_SIZE {
                self.pes[r][c].clear();
            }
        }

        // DMA 로드 시작
        self.dma.start_load(addr_a, addr_b);
        self.state = SystolicState::Loading;
    }

    pub fn step(&mut self, bus: &mut SystemBus) {
        self.global_time = self.global_time.wrapping_add(1);

        match self.state {
            SystolicState::Idle | SystolicState::Done => {}

            SystolicState::Loading => {
                self.dma.step(bus, &mut self.scratchpad);

                if self.dma.state == DmaState::Done {
                    self.cycle = 0;
                    self.state = SystolicState::Computing;
                }
            }

            SystolicState::Computing => {
                for row in (0..ARRAY_SIZE).rev() {
                    for col in (0..ARRAY_SIZE).rev() {
                        let a_in = if col == 0 {
                            self.scratchpad.a_input(row, self.cycle)
                        } else {
                            self.pes[row][col - 1].a_reg
                        };

                        let b_in = if row == 0 {
                            self.scratchpad.b_input(col, self.cycle)
                        } else {
                            self.pes[row - 1][col].b_reg
                        };

                        self.pes[row][col].step(a_in, b_in);
                    }
                }

                self.cycle += 1;

                let total_cycles = (ARRAY_SIZE * 2 - 2) + (INNER_DIM - 1) + MAC_LATENCY;
                if self.cycle >= total_cycles {
                    self.dma.start_store(self.addr_c);
                    self.state = SystolicState::Storing;
                }
            }

            SystolicState::Storing => {
                if let Some((row, col)) = self.dma.store_position() {
                    let temp_block = [
                        self.pes[row][col].psum as u32,
                        self.pes[row][col + 1].psum as u32,
                        self.pes[row][col + 2].psum as u32,
                        self.pes[row][col + 3].psum as u32,
                    ];

                    self.dma.store_step(bus, temp_block);
                }

                if self.dma.state == DmaState::Done {
                    self.state = SystolicState::Done;
                    self.status = 2;
                }
            }
        }
    }
}

impl Default for SystolicArray {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::hardware::soc::memory::{DRAM_BASE, Dram};
    use crate::hardware::soc::on_chip_bus::{BusOwner, SystemBus};

    #[test]
    fn test_systolic_array_mac() {
        const MEMORY_SIZE: usize = 64 * 1024;
        const MAX_CYCLES: usize = 10_000;

        // DMA와 SystemBus에는 물리 주소를 전달한다.
        const ADDR_A: u32 = DRAM_BASE + 0x1000;
        const ADDR_B: u32 = DRAM_BASE + 0x2000;
        const ADDR_C: u32 = DRAM_BASE + 0x3000;

        let mut dram = Dram::new(MEMORY_SIZE);
        let mut systolic = SystolicArray::new();

        // ------------------------------------------------------------
        // 1. 입력 행렬 초기화
        // ------------------------------------------------------------

        // A: ARRAY_SIZE × INNER_DIM
        // 모든 원소를 1로 초기화한다.
        for row in 0..ARRAY_SIZE {
            for col in 0..INNER_DIM {
                let offset = ((row * INNER_DIM + col) * 4) as u32;

                dram.load_firmware(ADDR_A + offset, &1_u32.to_le_bytes());
            }
        }

        // B: INNER_DIM × ARRAY_SIZE
        // 모든 원소를 1로 초기화한다.
        for row in 0..INNER_DIM {
            for col in 0..ARRAY_SIZE {
                let offset = ((row * ARRAY_SIZE + col) * 4) as u32;

                dram.load_firmware(ADDR_B + offset, &1_u32.to_le_bytes());
            }
        }

        // 출력 쓰기가 수행됐는지도 확인할 수 있도록
        // 결과 영역을 예상 결과와 다른 값으로 초기화한다.
        for row in 0..ARRAY_SIZE {
            for col in 0..ARRAY_SIZE {
                let offset = ((row * ARRAY_SIZE + col) * 4) as u32;

                dram.load_firmware(ADDR_C + offset, &u32::MAX.to_le_bytes());
            }
        }

        // ------------------------------------------------------------
        // 2. DMA 및 연산 시작
        // ------------------------------------------------------------

        systolic.start(ADDR_A, ADDR_B, ADDR_C);

        let mut total_cycles = 0;

        while systolic.status != 2 && total_cycles < MAX_CYCLES {
            // 실제 SoC와 같은 순서:
            // DRAM 시간 진행 → 버스 생성 → Systolic 진행.
            dram.tick();

            {
                // 테스트에는 Systolic DMA만 존재한다.
                // 현재 SoC처럼 사이클 시작 시 owner를 초기화한다.
                let mut internal_mmio = [];
                let mut external_mmio = [];

                let mut bus = SystemBus::new(
                    BusOwner::None,
                    &mut dram,
                    &mut internal_mmio,
                    &mut external_mmio,
                );

                systolic.step(&mut bus);
            }

            total_cycles += 1;
        }

        assert!(
            systolic.status == 2,
            "Systolic 작업 시간 초과: {}사이클, status={}",
            total_cycles,
            systolic.status,
        );

        // ------------------------------------------------------------
        // 3. 결과 검증
        // ------------------------------------------------------------

        // A와 B의 모든 원소가 1이므로:
        // C[row][col] = INNER_DIM
        let expected = INNER_DIM as u32;

        for row in 0..ARRAY_SIZE {
            for col in 0..ARRAY_SIZE {
                let offset = ((row * ARRAY_SIZE + col) * 4) as u32;

                // Dram::load32()는 물리 주소가 아닌
                // DRAM 시작점 기준 상대 주소를 받는다.
                let dram_offset = (ADDR_C - DRAM_BASE) + offset;

                let actual = dram.load32(dram_offset);

                assert_eq!(
                    actual, expected,
                    "C[{}][{}] 불일치: 예상={}, 실제={}",
                    row, col, expected, actual,
                );
            }
        }

        println!("Systolic MAC 검증 완료: {}사이클", total_cycles,);
    }
}
