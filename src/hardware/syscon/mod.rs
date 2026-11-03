pub mod mmio;

pub const SYSCON_BASE: u32 = 0x0010_0000;
pub const SYSCON_SIZE: u32 = 0x1000;

pub struct Syscon {
    pub is_halted: bool, // 시스템 종료 상태를 나타내는 플래그
    pub exit_code: u32,  // 시스템 종료 코드
}

impl Syscon {
    pub fn new() -> Self {
        Self {
            is_halted: false,
            exit_code: 0,
        }
    }
}

impl Default for Syscon {
    fn default() -> Self {
        Self::new()
    }
}
