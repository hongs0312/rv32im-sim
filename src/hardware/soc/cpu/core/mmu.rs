use crate::{
    hardware::soc::cpu::core::{PrivilegeMode, Trap},
    sim_log,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Instruction, // 명령어 인출(IF 단계)
    Load,        // 메모리 읽기 (MEM 단계)
    Store,       // 메모리 쓰기 (MEM 단계)
}

// PTE (Page Table Entry) 플래그 상수
const PAGE_V: u32 = 1 << 0; // Valid
const PAGE_R: u32 = 1 << 1; // Read
const PAGE_W: u32 = 1 << 2; // Write
const PAGE_X: u32 = 1 << 3; // Execute
const PAGE_U: u32 = 1 << 4; // User
// const PAGE_A: u32 = 1 << 6; // Accessed
// const PAGE_D: u32 = 1 << 7; // Dirty

const PAGE_SIZE: u32 = 4096; // 페이지 크기 (4KB)

pub struct Mmu;

impl Mmu {
    /// 가상 주소(vaddr)를 물리 주소(paddr)로 변환합니다.
    /// 실패 시 RIS-V 표준 Page Fault Trap을 반환합니다.
    pub fn translate<F>(
        vaddr: u32,
        access_type: AccessType,
        priv_mode: PrivilegeMode,
        satp: u32,
        sstatus: u32,
        mut read_phys_u32: F, // 물리 메모리(Bus/DRAM)에서 4바이트를 읽는 클로저
    ) -> Result<u32, Trap>
    where
        F: FnMut(u32) -> Result<u32, String>, // 물리 메모리 읽기 클로저의 시그니처
    {
        // M-mode이거나 satp의 MODE 비트가 0이면, 가상 주소를 그대로 물리 주소로 사용합니다.
        let mode_sv32 = (satp >> 31) & 1 == 1;
        if priv_mode == PrivilegeMode::Machine || !mode_sv32 {
            return Ok(vaddr);
        }

        // 가상 주소에서 VPN[1], VPN[0], page offset 추출
        let vpn = [
            (vaddr >> 12) & 0x3FF, // VPN[0] (10 bits)
            (vaddr >> 22) & 0x3FF, // VPN[1] (10 bits)
        ];
        let page_offset = vaddr & 0xFFF; // 페이지 오프셋 (12 bits)

        // 루트(1단계) 페이지 테이블의 베이스 주소 계산
        let mut a = (satp & 0x3FFFFF) * PAGE_SIZE;
        let mut i: isize = 1; // Level 1부터 탐색 시작
        let mut pte: u32 = 0;

        // 2단계 Hardware Page Table Walk
        while i >= 0 {
            let pte_addr = a.wrapping_add(vpn[i as usize] * 4); // PTE는 4바이트

            pte = match read_phys_u32(pte_addr) {
                Ok(val) => val,
                Err(_) => return Err(Self::access_fault(access_type, vaddr)), // Page Fault
            };

            let v = (pte & PAGE_V) != 0;
            let r = (pte & PAGE_R) != 0;
            let w = (pte & PAGE_W) != 0;
            let x = (pte & PAGE_X) != 0;

            // V = 0 이거나 (R=0, W=1)인 경우(예약된 잘못된 비트 조합) -> Page Fault
            if !v || (!r && w) {
                sim_log!(
                    "[TRAP!] Invalid PTE: pte: 0x{:08X}, access_type: {:?}, vaddr: 0x{:08X}\n",
                    pte,
                    access_type,
                    vaddr
                );
                return Err(Self::page_fault(access_type, vaddr)); // Invalid PTE
            }

            // R, W, X중 하나라도 1이면 리프(leaf) PTE에 도달한 것
            // !!! 이 부분 이해 안됨...
            if r || w || x {
                break; // 리프 PTE 발견
            }

            // 리프가 아니면 다음 단계(Level 0) 페이지 테이블로 이동
            i -= 1;
            if i < 0 {
                // Level 0까지 왔는데도 리프가 아니면 Page Fault
                return Err(Self::page_fault(access_type, vaddr));
            }

            let ppn = (pte >> 10) & 0x3FFFFF; // PPN[1:0] 추출
            a = ppn * PAGE_SIZE; // 다음 단계 페이지 테이블의 물리 주소
        }

        // 권한 검사(Permission Check)
        let r = (pte & PAGE_R) != 0;
        let w = (pte & PAGE_W) != 0;
        let x = (pte & PAGE_X) != 0;
        let u = (pte & PAGE_U) != 0;

        // U-mode 접근 권한 검사
        if priv_mode == PrivilegeMode::User && !u {
            return Err(Self::page_fault(access_type, vaddr)); // User mode 접근 불가
        }

        // S-mode에서 U=1 페이지 접근 검사 (sstatus.SUM 비트[18]가 0이면 접근 불가)
        // 단, S-mode는 SUM 비트와 무관하게 U=1 페이지의 명령어를 실행(Instruction Fetch)할 수 없음
        if priv_mode == PrivilegeMode::Supervisor && u {
            if access_type == AccessType::Instruction {
                return Err(Self::page_fault(access_type, vaddr)); // S-mode에서 U=1 페이지 접근 불가
            }
            let sum = (sstatus >> 18) & 1 != 0;
            if !sum {
                return Err(Self::page_fault(access_type, vaddr)); // S-mode에서 U=1 페이지 접근 불가
            }
        }

        // 접근 타입별 R/W/X 권한 검사
        match access_type {
            AccessType::Instruction if !x => return Err(Self::page_fault(access_type, vaddr)), // Execute 권한 없음
            AccessType::Load if !r => return Err(Self::page_fault(access_type, vaddr)), // Read 권한 없음
            AccessType::Store if !w => return Err(Self::page_fault(access_type, vaddr)), // Write 권한 없음
            _ => {}
        }

        // 물리 주소 조립 (Superpage 처리 포함)
        let ppm = (pte >> 10) & 0x3FFFFF; // PPN[1:0] 추출
        let paddr = if i > 0 {
            // i == 1에서 멈췄다면 4MB Mega Page (Superpage)
            // 하위 10비트 PPN[0]이 0으로 정렬되어 있지 않으면 Page Fault
            if (ppm & 0x3FF) != 0 {
                return Err(Self::page_fault(access_type, vaddr)); // Misaligned Superpage
            }
            let ppn1 = ppm >> 10; // PPN[1] 추출
            (ppn1 << 22) | (vpn[0] << 12) | page_offset
        } else {
            // 일반적인 4KB 페이지
            (ppm << 12) | page_offset
        };

        Ok(paddr)
    }

    fn page_fault(access_type: AccessType, vaddr: u32) -> Trap {
        let cause = match access_type {
            AccessType::Instruction => 12, // Instruction page fault
            AccessType::Load => 13,        // Load page fault
            AccessType::Store => 15,       // Store/AMO page fault
        };
        Trap { cause, tval: vaddr }
    }

    fn access_fault(access_type: AccessType, vaddr: u32) -> Trap {
        let cause = match access_type {
            AccessType::Instruction => 1, // Instruction access fault
            AccessType::Load => 5,        // Load access fault
            AccessType::Store => 7,       // Store/AMO access fault
        };
        Trap { cause, tval: vaddr }
    }
}
