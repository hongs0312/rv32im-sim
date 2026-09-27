pub struct Dram {
    pub dram: Vec<u8>,
}

impl Dram {
    pub fn new(size: usize) -> Self {
        Self {
            dram: vec![0; size],
        }
    }

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

    // 캐시에 블록 단위로 데이터를 로드하는 메서드
    pub fn load_block(&self, addr: usize) -> [u8; 16] {
        let mut block = [0u8; 16];
        block.copy_from_slice(&self.dram[addr..addr + 16]);

        block
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

    // 캐시에 블록 단위로 데이터를 저장하는 메서드
    pub fn store_block(&mut self, addr: usize, block: &[u8; 16]) {
        self.dram[addr..addr + 16].copy_from_slice(block);
    }
}
