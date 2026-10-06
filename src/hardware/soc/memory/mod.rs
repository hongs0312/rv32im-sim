use crate::hardware::soc::types::OpStatus;

pub mod bank;

use self::bank::{MemoryBank, ROW_SIZE};

const NUM_BANKS: usize = 16;

pub struct Dram {
    pub banks: [MemoryBank; NUM_BANKS],
}

impl Dram {
    /// 전체 용량을 받아 16개의 뱅크로 균등 분할하여 생성
    pub fn new(total_capacity: usize) -> Self {
        // 16개의 뱅크가 각각 나누어 가질 용량 계산
        let bank_capacity = total_capacity / NUM_BANKS;
        let num_rows = bank_capacity / ROW_SIZE;

        Self {
            banks: core::array::from_fn(|_| MemoryBank::new(num_rows)),
        }
    }

    pub fn tick(&mut self) {
        for bank in &mut self.banks.iter_mut() {
            bank.tick();
        }
    }

    pub fn issue_read(&mut self, addr: u32) -> OpStatus<()> {
        let (bank_id, row, offset) = Self::decode_address(addr);

        self.banks[bank_id].issue_read(row, offset)
    }

    pub fn collect_read(&mut self, addr: u32) -> OpStatus<[u8; 16]> {
        let (bank_id, row, offset) = Self::decode_address(addr);

        self.banks[bank_id].collect_read(row, offset)
    }

    pub fn issue_write(&mut self, addr: u32, block: &[u8; 16]) -> OpStatus<()> {
        let (bank_id, row, offset) = Self::decode_address(addr);

        self.banks[bank_id].issue_write(row, offset, block)
    }

    pub fn collect_write(&mut self, addr: u32) -> OpStatus<()> {
        let (bank_id, row, offset) = Self::decode_address(addr);

        self.banks[bank_id].collect_write(row, offset)
    }

    /// 32비트 물리 주소를 파싱하여 뱅크 인터리빙을 수행하는 주소 디코더
    fn decode_address(addr: u32) -> (usize, usize, usize) {
        // 1. 하위 4비트(16바이트 오프셋)를 버려 블록 단위 주소로 변환
        let block_addr = (addr >> 4) as usize;

        // 2. Fine-grained Interleaving: 블록 주소의 하위 4비트로 뱅크 ID(0~15) 결정
        let bank_id = block_addr & 0xF;

        // 3. 뱅크 내부 주소 계산
        let bank_internal_index = block_addr >> 4;

        // Row 하나는 1024바이트 (64블록)이므로 6비트 쉬프트
        let row = bank_internal_index >> 6;
        let offset = (bank_internal_index & 0x3F) * 16;

        (bank_id, row, offset)
    }

    // --- (디버깅용) CPU 부팅 전 C 바이너리(ELF)를 메모리에 직접 적재할 때 사용하는 백도어 ---
    // 시뮬레이터 초기화 단계에서만 사용되며, 타이밍 딜레이를 무시하고 뱅크에 직접 씁니다.
    pub fn load_firmware(&mut self, addr: u32, data: &[u8]) {
        for (i, &byte) in data.iter().enumerate() {
            let target_addr = addr + i as u32;
            let (bank_id, row, offset) = Self::decode_address(target_addr);

            // 뱅크 내부의 1차원 배열에 직접 접근하여 바이트 단위로 쓰기
            let base = row * ROW_SIZE + offset;

            // 바이트 단위 오프셋 보정 (블록 오프셋 + 블록 내 바이트 오프셋)
            let byte_offset = (target_addr & 0xF) as usize;
            self.banks[bank_id].data[base + byte_offset] = byte;
        }
    }

    pub fn load32(&self, addr: u32) -> u32 {
        let b0 = self.debug_read_byte(addr);
        let b1 = self.debug_read_byte(addr + 1);
        let b2 = self.debug_read_byte(addr + 2);
        let b3 = self.debug_read_byte(addr + 3);
        u32::from_le_bytes([b0, b1, b2, b3])
    }

    fn debug_read_byte(&self, addr: u32) -> u8 {
        let (bank_id, row, offset) = Self::decode_address(addr);
        let base = row * ROW_SIZE + offset;
        let byte_offset = (addr & 0xF) as usize; // 블록 내의 바이트 위치

        self.banks[bank_id].data[base + byte_offset]
    }
}
