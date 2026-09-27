// 16바이트 크기의 캐시 라인
const CACHE_LINE_SIZE: usize = 16;

#[derive(Clone, Copy)]
pub struct CacheLine {
    pub valid: bool,
    pub dirty: bool,
    pub tag: u32,
    pub data: [u8; CACHE_LINE_SIZE],
}

impl CacheLine {
    pub fn new() -> Self {
        Self {
            valid: false,
            dirty: false,
            tag: 0,
            data: [0; CACHE_LINE_SIZE],
        }
    }

    // 16바이트 블록에서 특정 오프셋의 32비트 데이터를 읽음
    pub fn read32(&self, offset: usize) -> u32 {
        let bytes = &self.data[offset..offset + 4];
        u32::from_le_bytes(bytes.try_into().expect("Slice with incorrect length"))
    }

    // 16바이트 블록에서 특정 오프셋에 32비트 데이터를 씀
    pub fn write32(&mut self, offset: usize, value: u32) {
        let offset = (offset % 16) as usize;
        let size = 4; // Word 접근일 경우

        // 캐시 라인(16바이트) 경계를 넘어가는지 검사
        if offset + size > 16 {
            // 실제라면 Exception 처리를 해야 하지만, 시뮬레이터 안정성을 위해 에러 로그만 띄우고 무시 (또는 강제 정렬)
            // tui 모드에서는 print가 깨질 수 있으니, 0을 반환하거나 그냥 return 하도록 처리합니다.
            return; // 읽기 실패 시 더미 값 반환
        }

        let bytes = value.to_le_bytes();
        self.data[offset..offset + 4].copy_from_slice(&bytes);
        self.dirty = true; // 데이터가 변경되었음을 표시
    }

    pub fn write16(&mut self, offset: usize, value: u16) {
        let bytes = value.to_le_bytes();
        self.data[offset..offset + 2].copy_from_slice(&bytes);
        self.dirty = true; // 데이터가 변경되었음을 표시
    }

    pub fn write8(&mut self, offset: usize, value: u8) {
        self.data[offset] = value;
        self.dirty = true; // 데이터가 변경되었음을 표시
    }
}
