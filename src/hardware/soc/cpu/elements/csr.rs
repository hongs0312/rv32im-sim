#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PrivilegeMode {
    User = 0b00,       // 일반 애플리케이션 모드 (User Mode)
    Supervisor = 0b01, // OS 커널 모드 (Kernel Mode)
    Machine = 0b11,    // 최상위 하드웨어/펌웨어 모드 (Machine Mode)
}

impl PrivilegeMode {
    pub fn from_bits(bits: u32) -> Self {
        match bits & 0b11 {
            0b00 => Self::User,
            0b01 => Self::Supervisor,
            _ => Self::Machine,
        }
    }
}

// CSR 주소 상수 (RISC-V Privileged Spec)
pub const CSR_MSTATUS: u16 = 0x300; // Machine Status
pub const CSR_MIE: u16 = 0x304; // Machine Interrupt Enable
pub const CSR_MTVEC: u16 = 0x305; // Machine Trap-Vector Base-Address
pub const CSR_MSCRATCH: u16 = 0x340; // Machine Scratch (OS 문맥 교환용 임시 레지스터)
pub const CSR_MEPC: u16 = 0x341; // Machine Exception Program Counter
pub const CSR_MCAUSE: u16 = 0x342; // Machine Trap Cause
pub const CSR_MTVAL: u16 = 0x343; // Machine Trap Value
pub const CSR_MIP: u16 = 0x344; // Machine Interrupt Pending

// mstatus 비트 마스크
const MSTATUS_MIE: u32 = 1 << 3; // 전역 인터럽트 활성화 비트
const MSTATUS_MPIE: u32 = 1 << 7; // 트랩 진입 전 MIE 백업 비트
const MSTATUS_MPP_SHIFT: u32 = 11;
const MSTATUS_MPP_MASK: u32 = 0b11 << MSTATUS_MPP_SHIFT; // 트랩 진입 전 Mode 백업 비트

// mie / mip 인터럽트 비트 마스크
pub const INT_M_SOFT: u32 = 1 << 3; // 소프트웨어 인터럽트 (MSIP/MSIE)
pub const INT_M_TIMER: u32 = 1 << 7; // 타이머 인터럽트 (MTIP/MTIE)
pub const INT_M_EXT: u32 = 1 << 11; // 외부 장치 인터럽트 (MEIP/MEIE - 가속기/PLIC용)

// mcause 원인 코드 (최상위 비트가 1이면 비동기 인터럽트, 0이면 동기 예외)
pub const CAUSE_INTERRUPT_BIT: u32 = 1 << 31;
pub const CAUSE_M_SOFT_INT: u32 = CAUSE_INTERRUPT_BIT | 3;
pub const CAUSE_M_TIMER_INT: u32 = CAUSE_INTERRUPT_BIT | 7;
pub const CAUSE_M_EXT_INT: u32 = CAUSE_INTERRUPT_BIT | 11;

pub struct CsrFile {
    pub mode: PrivilegeMode, // 현재 CPU의 Mode Bit
    pub mstatus: u32,
    pub mie: u32,
    pub mtvec: u32,
    pub mscratch: u32,
    pub mepc: u32,
    pub mcause: u32,
    pub mtval: u32,
    pub mip: u32,
}

impl CsrFile {
    pub const fn new() -> Self {
        Self {
            mode: PrivilegeMode::Machine, // 부팅 시 항상 M-mode로 시작
            mstatus: 0, 
            mie: 0,
            mtvec: 0,
            mscratch: 0,
            mepc: 0,
            mcause: 0,
            mtval: 0,
            mip: 0,
        }
    }

    /// 1. 매 클럭 외부 하드웨어(CLINT, Systolic/PLIC)의 IRQ 배선 상태를 업데이트합니다.
    pub fn update_interrupt_pins(&mut self, sw_irq: bool, timer_irq: bool, ext_irq: bool) {
        let mut pending = 0;
        if sw_irq {
            pending |= INT_M_SOFT;
        }
        if timer_irq {
            pending |= INT_M_TIMER;
        }
        if ext_irq {
            pending |= INT_M_EXT;
        }
        self.mip = pending;
    }

    /// 2. 현재 클럭에 처리해야 할 인터럽트가 있는지 확인하고, 있다면 원인을 반환합니다.
    pub fn check_pending_interrupt(&self) -> Option<u32> {
        // M-mode에서는 mstatus.MIE가 1일 때만 인터럽트 허용.
        // U-mode(유저 프로그램 실행 중)에서는 M-mode 인터럽트를 무조건 즉시 허용!
        let global_enabled =
            (self.mode != PrivilegeMode::Machine) || ((self.mstatus & MSTATUS_MIE) != 0);

        if !global_enabled {
            return None;
        }

        let active_irqs = self.mip & self.mie;
        if active_irqs == 0 {
            return None;
        }

        // RISC-V 표준 인터럽트 우선순위: External > Software > Timer
        if (active_irqs & INT_M_EXT) != 0 {
            Some(CAUSE_M_EXT_INT)
        } else if (active_irqs & INT_M_SOFT) != 0 {
            Some(CAUSE_M_SOFT_INT)
        } else if (active_irqs & INT_M_TIMER) != 0 {
            Some(CAUSE_M_TIMER_INT)
        } else {
            None
        }
    }

    /// 3. 인터럽트 또는 예외(ecall 등) 발생 시 하드웨어 상태를 전환하고 점프할 주소를 반환합니다.
    pub fn enter_trap(&mut self, cause: u32, epc: u32, tval: u32) -> u32 {
        self.mepc = epc;
        self.mcause = cause;
        self.mtval = tval;

        // 현재 MIE 비트를 MPIE에 백업하고, MIE를 0으로 꺼서 중첩 인터럽트 방지
        let current_mie = (self.mstatus & MSTATUS_MIE) != 0;
        if current_mie {
            self.mstatus |= MSTATUS_MPIE;
        } else {
            self.mstatus &= !MSTATUS_MPIE;
        }
        self.mstatus &= !MSTATUS_MIE;

        // 현재 Mode Bit를 MPP에 백업하고, 강제로 Machine 모드로 승격!
        self.mstatus &= !MSTATUS_MPP_MASK;
        self.mstatus |= (self.mode as u32) << MSTATUS_MPP_SHIFT;
        self.mode = PrivilegeMode::Machine;

        // mtvec 계산 (하위 2비트가 0이면 Direct 모드, 1이면 Vectored 모드)
        let base = self.mtvec & !0b11;
        let mode = self.mtvec & 0b11;
        let is_interrupt = (cause & CAUSE_INTERRUPT_BIT) != 0;

        if mode == 1 && is_interrupt {
            let code = cause & !CAUSE_INTERRUPT_BIT;
            base.wrapping_add(code * 4)
        } else {
            base
        }
    }

    /// 4. OS가 `mret` 명령어를 실행해 인터럽트에서 복귀할 때 호출됩니다.
    pub fn execute_mret(&mut self) -> u32 {
        // MPIE를 MIE로 복원하고, MPIE는 1로 세팅
        let prev_mie = (self.mstatus & MSTATUS_MPIE) != 0;
        if prev_mie {
            self.mstatus |= MSTATUS_MIE;
        } else {
            self.mstatus &= !MSTATUS_MIE;
        }
        self.mstatus |= MSTATUS_MPIE;

        // MPP에 저장되어 있던 이전 Mode Bit(예: User)로 권한 강등!
        let mpp_bits = (self.mstatus & MSTATUS_MPP_MASK) >> MSTATUS_MPP_SHIFT;
        self.mode = PrivilegeMode::from_bits(mpp_bits);

        // MPP 필드는 가장 낮은 권한(User)으로 초기화
        self.mstatus &= !MSTATUS_MPP_MASK;

        // 원래 실행하려던 명령어 주소 반환
        self.mepc
    }

    /// CSR 읽기/쓰기 명령어(`csrrw`, `csrrs`, `csrrc`) 지원용 메서드
    pub fn read(&self, addr: u16) -> Result<u32, String> {
        // 특권 권한 검사: M-mode CSR(0x300 대역)은 User 모드에서 접근 시 Illegal Instruction 예외!
        if self.mode != PrivilegeMode::Machine && (addr & 0x300) == 0x300 {
            return Err("Privilege violation: Accessing M-mode CSR from lower mode".into());
        }
        match addr {
            CSR_MSTATUS => Ok(self.mstatus),
            CSR_MIE => Ok(self.mie),
            CSR_MTVEC => Ok(self.mtvec),
            CSR_MSCRATCH => Ok(self.mscratch),
            CSR_MEPC => Ok(self.mepc),
            CSR_MCAUSE => Ok(self.mcause),
            CSR_MTVAL => Ok(self.mtval),
            CSR_MIP => Ok(self.mip),
            _ => Ok(0),
        }
    }

    pub fn write(&mut self, addr: u16, value: u32) -> Result<(), String> {
        if self.mode != PrivilegeMode::Machine && (addr & 0x300) == 0x300 {
            return Err("Privilege violation: Writing M-mode CSR from lower mode".into());
        }
        match addr {
            CSR_MSTATUS => self.mstatus = value,
            CSR_MIE => self.mie = value,
            CSR_MTVEC => self.mtvec = value,
            CSR_MSCRATCH => self.mscratch = value,
            CSR_MEPC => self.mepc = value,
            CSR_MCAUSE => self.mcause = value,
            CSR_MTVAL => self.mtval = value,
            // mip는 하드웨어 배선 전용이므로 소프트웨어 쓰기 무시 (MSIP는 CLINT MMIO로 제어)
            CSR_MIP => {}
            _ => {}
        }
        Ok(())
    }
}

impl Default for CsrFile {
    fn default() -> Self {
        Self::new()
    }
}
