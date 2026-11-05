#![cfg_attr(rustfmt, rustfmt_skip)]

use crate::{hardware::soc::cpu::core::{PrivilegeMode, Trap}};

// ============================================================================
// CSR 주소 상수 (RISC-V Privileged Spec)
// ============================================================================
// Machine-Level CSRs
pub const CSR_MHARTID: u16 = 0xF14;  // Hardware Thread ID
pub const CSR_MIP: u16 = 0x344;      // Machine Interrupt Pending
pub const CSR_MIE: u16 = 0x304;      // Machine Interrupt Enable
pub const CSR_MEDELEG: u16 = 0x302;  // Machine Exception Delegation
pub const CSR_MIDELEG: u16 = 0x303;  // Machine Interrupt Delegation
pub const CSR_MSTATUS: u16 = 0x300;  // Machine Status
pub const CSR_MTVEC: u16 = 0x305;    // Machine Trap-Vector Base-Address
pub const CSR_MSCRATCH: u16 = 0x340; // Machine Scratch
pub const CSR_MEPC: u16 = 0x341;     // Machine Exception Program Counter
pub const CSR_MCAUSE: u16 = 0x342;   // Machine Trap Cause
pub const CSR_MTVAL: u16 = 0x343;    // Machine Trap Value


// Supervisor-Level CSRs
pub const CSR_SSTATUS: u16 = 0x100;  // Supervisor Status
pub const CSR_SIE: u16 = 0x104;      // Supervisor Interrupt Enable
pub const CSR_STVEC: u16 = 0x105;    // Supervisor Trap-Vector Base-Address
pub const CSR_SSCRATCH: u16 = 0x140; // Supervisor Scratch
pub const CSR_SEPC: u16 = 0x141;     // Supervisor Exception Program Counter
pub const CSR_SCAUSE: u16 = 0x142;   // Supervisor Trap Cause
pub const CSR_STVAL: u16 = 0x143;    // Supervisor Trap Value
pub const CSR_SIP: u16 = 0x144;      // Supervisor Interrupt Pending
pub const CSR_SATP: u16 = 0x180;     // Supervisor Address Translation and Protection


// ============================================================================
// 상태 및 인터럽트 비트 마스크
// ============================================================================
// mstatus 내 Machine 모드 제어 비트
const MSTATUS_MIE: u32 = 1 << 3;
const MSTATUS_MPIE: u32 = 1 << 7;
const MSTATUS_MPP_SHIFT: u32 = 11;
const MSTATUS_MPP_MASK: u32 = 0b11 << MSTATUS_MPP_SHIFT;

// mstatus 내 Supervisor 모드 제어 비트 (sstatus는 mstatus의 부분집합 뷰)
const MSTATUS_SIE: u32 = 1 << 1;
const MSTATUS_SPIE: u32 = 1 << 5;
const MSTATUS_SPP: u32 = 1 << 8;
const SSTATUS_MASK: u32 = 0x800DE122; // sstatus 접근 시 노출/수정 허용 마스크

// mie / mip 인터럽트 비트 마스크
pub const INT_S_SOFT: u32 = 1 << 1;
pub const INT_M_SOFT: u32 = 1 << 3;
pub const INT_S_TIMER: u32 = 1 << 5;
pub const INT_M_TIMER: u32 = 1 << 7;
pub const INT_S_EXT: u32 = 1 << 9;
pub const INT_M_EXT: u32 = 1 << 11;
const SIE_MASK: u32 = INT_S_SOFT | INT_S_TIMER | INT_S_EXT;

// mcause / scause 원인 코드
pub const CAUSE_INTERRUPT_BIT: u32 = 1 << 31;
pub const CAUSE_M_SOFT_INT: u32 = CAUSE_INTERRUPT_BIT | 3;
pub const CAUSE_M_TIMER_INT: u32 = CAUSE_INTERRUPT_BIT | 7;
pub const CAUSE_M_EXT_INT: u32 = CAUSE_INTERRUPT_BIT | 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsrFile {
    // 인터럽트 및 위임 레지스터
    pub mie: u32,
    pub mip: u32,
    pub medeleg: u32,
    pub mideleg: u32,

    // Machine 모드 트랩 레지스터
    pub mstatus: u32,
    pub mtvec: u32,
    pub mscratch: u32,
    pub mepc: u32,
    pub mcause: u32,
    pub mtval: u32,

    // Supervisor 모드 트랩 레지스터
    pub stvec: u32,
    pub sscratch: u32,
    pub sepc: u32,
    pub scause: u32,
    pub stval: u32,
    pub satp: u32,      // Supervisor Address Translation and Protection
}

impl CsrFile {
    pub const fn new() -> Self {
        Self {
            mstatus: 0,
            mie: 0,
            mip: 0,
            medeleg: 0,
            mideleg: 0,
            mtvec: 0,
            mscratch: 0,
            mepc: 0,
            mcause: 0,
            mtval: 0,
            stvec: 0,
            sscratch: 0,
            sepc: 0,
            scause: 0,
            stval: 0,
            satp: 0,
        }
    }

    /// mtvec / stvec 공통 점프 주소 계산 헬퍼 (Direct 모드 vs Vectored 모드)
    #[inline]
    fn calc_tvec_target(tvec: u32, cause: u32) -> u32 {
        let base = tvec & !0b11;
        let mode = tvec & 0b11;
        let is_interrupt = (cause & CAUSE_INTERRUPT_BIT) != 0;

        if mode == 1 && is_interrupt {
            let code = cause & !CAUSE_INTERRUPT_BIT;
            base.wrapping_add(code * 4)
        } else {
            base
        }
    }

    /// CSR 주소의 비트[9:8]에 인코딩된 최소 요구 권한을 현재 모드와 비교합니다.
    #[inline]
    fn check_privilege(&self, addr: u16, current_mode: PrivilegeMode) -> Result<(), String> {
        let required_priv = (addr >> 8) & 0b11;
        if (current_mode as u16) < required_priv {
            Err(format!(
                "Privilege violation: mode {:?} cannot access CSR 0x{:03x}",
                current_mode, addr
            ))
        } else {
            Ok(())
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
        self.mip = (self.mip & SIE_MASK) | pending;
    }

    /// 2. 현재 클럭에 처리해야 할 인터럽트가 있는지 확인하고, 있다면 원인을 반환합니다.
    pub fn check_pending_interrupt(&self, current_mode: PrivilegeMode) -> Option<u32> {
        let global_enabled =
            (current_mode != PrivilegeMode::Machine) || ((self.mstatus & MSTATUS_MIE) != 0);

        if !global_enabled {
            return None;
        }

        let active_irqs = self.mip & self.mie;
        if active_irqs == 0 {
            return None;
        }

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

    /// 3. 인터럽트 또는 예외 발생 시 하드웨어 상태를 전환하고 점프할 핸들러 주소를 반환합니다.
    pub fn enter_trap(&mut self, trap: Trap, epc: u32, current_mode: PrivilegeMode) -> (u32, PrivilegeMode) {
        let (cause, tval) = (trap.cause, trap.tval);
        
        let is_interrupt = (cause & CAUSE_INTERRUPT_BIT) != 0;
        let code = cause & !CAUSE_INTERRUPT_BIT;

        // M-mode보다 낮은 권한(S/U)에서 발생했고 위임 비트(medeleg/mideleg)가 켜져 있으면 S-mode로 위임
        let delegate_to_s = if current_mode < PrivilegeMode::Machine {
            let deleg = if is_interrupt { self.mideleg } else { self.medeleg };
            code < 32 && ((deleg >> code) & 1) != 0
        } else {
            false
        };

        if delegate_to_s {
            self.sepc = epc;
            self.scause = cause;
            self.stval = tval;

            // SIE -> SPIE 백업 및 SIE 비활성화
            if (self.mstatus & MSTATUS_SIE) != 0 {
                self.mstatus |= MSTATUS_SPIE;
            } else {
                self.mstatus &= !MSTATUS_SPIE;
            }
            self.mstatus &= !MSTATUS_SIE;

            // 현재 권한을 SPP(비트 8)에 백업 후 Supervisor 모드로 전환
            if current_mode == PrivilegeMode::User {
                self.mstatus &= !MSTATUS_SPP;
            } else {
                self.mstatus |= MSTATUS_SPP;
            }

            // Supervisor 모드 트랩 벡터 계산
            let target_pc = Self::calc_tvec_target(self.stvec, cause);
            (target_pc, PrivilegeMode::Supervisor)
        } else {
            self.mepc = epc;
            self.mcause = cause;
            self.mtval = tval;

            // sim_log!(
            //     "[CSR] Trap 발생: cause=0x{:08x}, tval=0x{:08x}, epc=0x{:08x}, current_mode={:?}",
            //     cause,
            //     tval,
            //     epc,
            //     current_mode
            // );

            // MIE -> MPIE 백업 및 MIE 비활성화
            if (self.mstatus & MSTATUS_MIE) != 0 {
                self.mstatus |= MSTATUS_MPIE;
            } else {
                self.mstatus &= !MSTATUS_MPIE;
            }
            self.mstatus &= !MSTATUS_MIE;

            // 현재 권한을 MPP(비트 12:11)에 백업 후 Machine 모드로 전환
            self.mstatus = (self.mstatus & !MSTATUS_MPP_MASK)
                | ((current_mode as u32) << MSTATUS_MPP_SHIFT);

            // Machine 모드 트랩 벡터 계산
            let target_pc = Self::calc_tvec_target(self.mtvec, cause);
            (target_pc, PrivilegeMode::Machine)
        }
    }

    /// 4-A. `mret` 명령어 실행 시 Machine 모드 트랩에서 복귀합니다.
    pub fn execute_mret(&mut self) -> (u32, PrivilegeMode) {
        if (self.mstatus & MSTATUS_MPIE) != 0 {
            self.mstatus |= MSTATUS_MIE;
        } else {
            self.mstatus &= !MSTATUS_MIE;
        }
        self.mstatus |= MSTATUS_MPIE;

        let mpp_bits = (self.mstatus & MSTATUS_MPP_MASK) >> MSTATUS_MPP_SHIFT;
        let new_mode = PrivilegeMode::from_bits(mpp_bits);
        self.mstatus &= !MSTATUS_MPP_MASK; // MPP를 User(0b00)로 초기화
        
        (self.mepc, new_mode)
    }

    /// 4-B. `sret` 명령어 실행 시 Supervisor 모드 트랩에서 복귀합니다.
    pub fn execute_sret(&mut self) -> (u32, PrivilegeMode) {
        if (self.mstatus & MSTATUS_SPIE) != 0 {
            self.mstatus |= MSTATUS_SIE;
        } else {
            self.mstatus &= !MSTATUS_SIE;
        }
        self.mstatus |= MSTATUS_SPIE;

        let new_mode = if (self.mstatus & MSTATUS_SPP) != 0 {
            PrivilegeMode::Supervisor
        } else {
            PrivilegeMode::User
        };
        self.mstatus &= !MSTATUS_SPP; // SPP를 User(0)로 초기화

        (self.sepc, new_mode)
    }

    /// 5. CSR 읽기 (`csrrw`, `csrrs`, `csrrc`)
    pub fn read(&self, addr: u16, current_mode: PrivilegeMode) -> Result<u32, String> {
        self.check_privilege(addr, current_mode)?;

        match addr {
            // S-Mode CSRs
            CSR_SSTATUS => Ok(self.mstatus & SSTATUS_MASK),     // sstatus는 mstatus의 부분집합
            CSR_SIE => Ok(self.mie & SIE_MASK),                 // sstatus는 mstatus의 부분집합
            CSR_STVEC => Ok(self.stvec),
            CSR_SSCRATCH => Ok(self.sscratch),
            CSR_SEPC => Ok(self.sepc),
            CSR_SCAUSE => Ok(self.scause),
            CSR_STVAL => Ok(self.stval),
            CSR_SIP => Ok(self.mip & SIE_MASK),                 // sip는 mip의 부분집합
            CSR_SATP => Ok(self.satp),

            // M-Mode CSRs
            CSR_MHARTID => Ok(0), // 단일 코어 SoC에서는 항상 0
            CSR_MSTATUS => Ok(self.mstatus),
            CSR_MEDELEG => Ok(self.medeleg),
            CSR_MIDELEG => Ok(self.mideleg),
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

    /// 6. CSR 쓰기 (`csrrw`, `csrrs`, `csrrc`)
    pub fn write(&mut self, addr: u16, value: u32, current_mode: PrivilegeMode) -> Result<(), String> {
        self.check_privilege(addr, current_mode)?;

        // RISC-V 스펙: 상위 2비트(비트 11:10)가 0b11이면 Read-Only CSR (예: 0xC00 cycle)
        if (addr & 0xC00) == 0xC00 {
            return Err(format!("Write to read-only CSR 0x{:03x}", addr));
        }

        match addr {
            // S-Mode CSRs
            CSR_SSTATUS => self.mstatus = (self.mstatus & !SSTATUS_MASK) | (value & SSTATUS_MASK),
            CSR_SIE => self.mie = (self.mie & !SIE_MASK) | (value & SIE_MASK),
            CSR_STVEC => self.stvec = value,
            CSR_SSCRATCH => self.sscratch = value,
            CSR_SEPC => self.sepc = value,
            CSR_SCAUSE => self.scause = value,
            CSR_STVAL => self.stval = value,
            CSR_SATP => self.satp = value,
            CSR_SIP => self.mip = (self.mip & !INT_S_SOFT) | (value & INT_S_SOFT),

            // M-Mode CSRs
            CSR_MSTATUS => self.mstatus = value,
            CSR_MEDELEG => self.medeleg = value,
            CSR_MIDELEG => self.mideleg = value,
            CSR_MIE => self.mie = value,
            CSR_MTVEC => self.mtvec = value,
            CSR_MSCRATCH => self.mscratch = value,
            CSR_MEPC => self.mepc = value,
            CSR_MCAUSE => self.mcause = value,
            CSR_MTVAL => self.mtval = value,
            CSR_MIP => self.mip = (self.mip & !SIE_MASK) | (value & SIE_MASK),
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