/*
    systolic array에 사용되는 scratchpad를 구현한 모듈
    DMA로부터 데이터를 직접 입력받아 저장(SRAM 역할)하며,
    사이클에 맞춰 Skewing(지연)을 적용해 PE로 방출(LineBuffer 역할)합니다.
*/

use super::StreamValue;
use super::{ARRAY_SIZE, INNER_DIM};

pub struct Scratchpad {
    // A 행렬용 SRAM (Input)
    pub a_data: [[u32; INNER_DIM]; ARRAY_SIZE],
    pub a_valid: [[bool; INNER_DIM]; ARRAY_SIZE], // 데이터가 들어오면 true, PE로 빠져나가면 false

    // B 행렬용 SRAM (Weight)
    pub b_data: [[u32; ARRAY_SIZE]; INNER_DIM],
    pub b_valid: [[bool; ARRAY_SIZE]; INNER_DIM],
}

impl Scratchpad {
    pub fn new() -> Self {
        Self {
            a_data: [[0; INNER_DIM]; ARRAY_SIZE],
            a_valid: [[false; INNER_DIM]; ARRAY_SIZE],
            b_data: [[0; ARRAY_SIZE]; INNER_DIM],
            b_valid: [[false; ARRAY_SIZE]; INNER_DIM],
        }
    }

    // --- 1. DMA가 버스에서 데이터를 받아 직접 꽂아넣는 포트 ---
    pub fn write_a(&mut self, row: usize, col: usize, value: u32) {
        self.a_data[row][col] = value;
        self.a_valid[row][col] = true;
    }

    pub fn write_b(&mut self, row: usize, col: usize, value: u32) {
        self.b_data[row][col] = value;
        self.b_valid[row][col] = true;
    }

    // --- 2. 하드웨어 데이터 패스: 사이클에 맞춰 PE 배열로 데이터 방출 ---

    pub fn a_input(&mut self, row: usize, cycle: usize) -> StreamValue {
        // Skewing: row번째 행은 row 사이클만큼 대기 후 출발
        if cycle >= row && cycle < row + INNER_DIM {
            let col = cycle - row; // 현재 사이클에서 읽어야 할 열 인덱스
            
            if self.a_valid[row][col] {
                self.a_valid[row][col] = false; // [핵심] PE로 방출되었으므로 스크래치패드에서 소모(비움) 처리!
                return StreamValue { value: self.a_data[row][col], valid: true };
            }
        }
        StreamValue { value: 0, valid: false }
    }

    pub fn b_input(&mut self, col: usize, cycle: usize) -> StreamValue {
        // Skewing: col번째 열은 col 사이클만큼 대기 후 출발
        if cycle >= col && cycle < col + INNER_DIM {
            let row = cycle - col; // 현재 사이클에서 읽어야 할 행 인덱스
            
            if self.b_valid[row][col] {
                self.b_valid[row][col] = false; // [핵심] PE로 방출되었으므로 비움 처리!
                return StreamValue { value: self.b_data[row][col], valid: true };
            }
        }
        StreamValue { value: 0, valid: false }
    }

    pub fn clear(&mut self) {
        // 데이터는 덮어씌워지므로 valid 비트만 0으로 초기화하면 됩니다.
        self.a_valid = [[false; INNER_DIM]; ARRAY_SIZE];
        self.b_valid = [[false; ARRAY_SIZE]; INNER_DIM];
    }
}