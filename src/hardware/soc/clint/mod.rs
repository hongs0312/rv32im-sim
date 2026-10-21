pub mod mmio;

pub const CLINT_BASE: u32 = 0x0200_0000;
pub const CLINT_SIZE: u32 = 0x001_0000; // 64 KB

const MSIP_OFFSET: u32 = 0x0000; // 소프트웨어 인터럽트 (4B)
const MTIMECMP_LO_OFFSET: u32 = 0x4000; // 알람 비교값 하위 32비트
const MTIMECMP_HI_OFFSET: u32 = 0x4004; // 알람 비교값 상위 32비트
const MTIME_LO_OFFSET: u32 = 0xBFF8; // 현재 타이머 하위 32비트
const MTIME_HI_OFFSET: u32 = 0xBFFC; // 현재 타이머 상위 32비트

pub struct Clint {
    pub msip: u32,     // 하위 1비트만 사용 (0: 인터럽트 없음, 1: 인터럽트 발생)
    pub mtime: u64,    // 매 클럭 증가하는 64비트 타이머
    pub mtimecmp: u64, // OS가 설정하는 다음 알람 시각
}

impl Clint {
    pub const fn new() -> Self {
        Self {
            msip: 0,
            mtime: 0,
            mtimecmp: u64::MAX, // 초기값은 무한대 (알람 없음)
        }
    }

    pub fn tick(&mut self) -> (bool, bool) {
        self.mtime = self.mtime.wrapping_add(1);

        let sw_irq = (self.msip & 1) != 0;
        let timer_irq = self.mtime >= self.mtimecmp;

        (sw_irq, timer_irq)
    }
}

impl Default for Clint {
    fn default() -> Self {
        Self::new()
    }
}
