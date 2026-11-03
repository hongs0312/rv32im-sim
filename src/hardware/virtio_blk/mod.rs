use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

// use crate::hardware::soc::system_bus::SystemBus;
use crate::hardware::soc::memory::Dram;

pub mod mmio;

pub const VIRTIO_BASE: u32 = 0x1000_1000;
pub const VIRTIO_SIZE: u32 = 0x1000;
pub const SECTOR_SIZE: u64 = 512;

pub struct VirtioBlk {
    pub disk_image: File,
    pub capacity: u64, // 총 섹터 수 (파일 크기 / 512)

    // Legacy MMIO 레지스터 상태들
    pub host_features: u32,
    pub guest_features: u32,
    pub guest_page_size: u32,
    pub queue_sel: u32,
    pub queue_num: u32,
    pub queue_align: u32,
    pub queue_pfn: u32,
    pub interrupt_status: u32,
    pub status: u32,

    // 💡 DMA 연동을 위한 상태 변수
    pub pending_notify: bool, // QueueNotify(0x050)가 울렸는지 여부
    pub last_avail_idx: u16,  // 에뮬레이터가 마지막으로 처리한 Avail Ring 인덱스
}

impl VirtioBlk {
    pub fn new(image_path: &str) -> Self {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(image_path)
            .expect("Failed to open disk image");

        let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);

        let capacity = file_size / SECTOR_SIZE;

        Self {
            disk_image: file,
            capacity,
            host_features: 0,
            guest_features: 0,
            guest_page_size: 4096, // 기본 페이지 크기 4KB
            queue_sel: 0,
            queue_num: 0,
            queue_align: 4096, // Legacy 기본 Align
            queue_pfn: 0,
            interrupt_status: 0,
            status: 0,
            pending_notify: false,
            last_avail_idx: 0,
        }
    }

    pub fn reset(&mut self) {
        self.guest_features = 0;
        self.guest_page_size = 4096;
        self.queue_sel = 0;
        self.queue_num = 0;
        self.queue_align = 4096;
        self.queue_pfn = 0;
        self.interrupt_status = 0;
        self.status = 0;
        self.pending_notify = false;
        self.last_avail_idx = 0;
    }

    // 💡 장치 스스로 큐 노티프이를 감지하고 디스크 I/O를 처리하는 메서드
    pub fn tick(&mut self) {
        if !self.pending_notify {
            return;
        }
        self.pending_notify = false;

        // 1. 디스크 이미지에서 데이터 읽기 (기능 검증용 0번 섹터 예시)
        let mut sector_buf = [0u8; 512];
        if self.disk_image.seek(SeekFrom::Start(0)).is_ok() {
            let _ = self.disk_image.read(&mut sector_buf);
        }

        // 2. (추후 정석 구현 시)
        // queue_base_paddr()를 통해 DRAM 주소를 계산하고,
        // SystemBus나 Dram 참조를 받아 여기에 sector_buf 내용을 써넣게 됩니다.

        // 3. 디스크 작업 완료 인터럽트 핀 활성화 (장치 스스로 상태 변경)
        self.set_used_buffer_notification();
    }

    // 💡 Hardware::tick_with에서 호출하는 인터럽트 핀 상태 확인 함수
    pub fn is_interrupting(&self) -> bool {
        (self.interrupt_status & 0x1) != 0
    }

    // 디스크 I/O(DMA) 완료 시 내부에서 호출할 헬퍼 함수
    pub fn set_used_buffer_notification(&mut self) {
        self.interrupt_status |= 0x1;
    }

    // 💡 큐의 시작 물리 주소를 계산하는 헬퍼 메서드 추가
    pub fn queue_base_paddr(&self) -> u32 {
        self.queue_pfn.wrapping_mul(self.guest_page_size)
    }

    // 💡 SystemBus 대신 순수 메모리인 Dram을 넘겨받아 빌림 충돌을 원천 차단합니다.
    pub fn process_sync_dma(&mut self, dram: &mut Dram) {
        if !self.pending_notify {
            return;
        }
        // 💡 주의: 여기서 self.pending_notify = false; 를 미리 하지 않습니다!

        let queue_base = self.queue_base_paddr();
        let qn = self.queue_num as u32;

        let read32 = |d: &Dram, paddr: u32| -> u32 { d.load32(paddr.saturating_sub(0x8000_0000)) };
        let read16 = |d: &Dram, paddr: u32| -> u16 {
            let val = read32(d, paddr & !3);
            let shift = (paddr % 4) * 8;
            ((val >> shift) & 0xFFFF) as u16
        };
        let write32 = |d: &mut Dram, paddr: u32, val: u32| {
            d.load_firmware(paddr, &val.to_le_bytes());
        };
        let write16 = |d: &mut Dram, paddr: u32, val: u16| {
            d.load_firmware(paddr, &val.to_le_bytes());
        };
        let write_buf = |d: &mut Dram, paddr: u32, buf: &[u8]| {
            d.load_firmware(paddr, buf);
        };

        let avail_idx_addr = queue_base + 16 * qn + 2;
        let kernel_avail_idx = read16(dram, avail_idx_addr);

        // 커널 인덱스가 업데이트된 것을 확인했을 때
        if self.last_avail_idx != kernel_avail_idx {
            // 💡 [핵심] 메모리에 데이터가 안착된 것을 확인했으므로 이제 플래그를 내립니다.
            self.pending_notify = false;

            // println!("  => Avail Index Addr: 0x{:08X}", avail_idx_addr);
            // println!("  => Emulator Last Avail: {}, Kernel Avail: {}", self.last_avail_idx, kernel_avail_idx);

            let ring_offset = queue_base + 16 * qn + 4 + 2 * (self.last_avail_idx as u32 % qn);
            let desc_head_idx = read16(dram, ring_offset);

            let desc0_addr = queue_base + 16 * (desc_head_idx as u32);
            let req_paddr = read32(dram, desc0_addr);
            let req_type = read32(dram, req_paddr);

            let sector_low = read32(dram, req_paddr + 8);
            let sector_high = read32(dram, req_paddr + 12);
            let sector = (sector_low as u64) | ((sector_high as u64) << 32);

            let desc1_addr = queue_base + 16 * ((desc_head_idx + 1) as u32);
            let data_paddr = read32(dram, desc1_addr);

            let desc2_addr = queue_base + 16 * ((desc_head_idx + 2) as u32);
            let status_paddr = read32(dram, desc2_addr);

            let data_len = read32(dram, desc1_addr + 8);

            let offset = sector * 512;
            let _ = self.disk_image.seek(SeekFrom::Start(offset));

            if req_type == 0 {
                // 읽기
                let mut sector_buf = [0u8; 512];
                let _ = self.disk_image.read(&mut sector_buf);
                write_buf(dram, data_paddr, &sector_buf);
                // println!("  => [I/O] 디스크 읽기 완료 및 RAM 복사 완료");
            } else if req_type == 1 {
                // 쓰기
                let mut write_buf = vec![0u8; data_len as usize];

                for i in 0..(data_len / 4) {
                    let word = read32(dram, data_paddr + i * 4);
                    write_buf[(i * 4) as usize..((i + 1) * 4) as usize]
                        .copy_from_slice(&word.to_le_bytes());
                }

                let remainder = data_len % 4;
                if remainder != 0 {
                    let word = read32(dram, data_paddr + (data_len - remainder));
                    let bytes = word.to_le_bytes();
                    let start_idx = (data_len - remainder) as usize;
                    for j in 0..remainder as usize {
                        write_buf[start_idx + j] = bytes[j];
                    }
                }

                // 디스크 이미지 파일에 덮어쓰기
                let _ = self.disk_image.write(&write_buf);

                // println!("  => [I/O] 디스크 쓰기 완료");
            }

            write_buf(dram, status_paddr, &[0u8]);

            let avail_end = queue_base + 16 * qn + 4 + 2 * qn;
            let align = self.queue_align;
            let used_ring_base = (avail_end + align - 1) & !(align - 1);

            let used_idx_addr = used_ring_base + 2;
            let current_used_idx = read16(dram, used_idx_addr);

            let slot_addr = used_ring_base + 4 + 8 * (current_used_idx as u32 % qn);
            write32(dram, slot_addr, desc_head_idx as u32);
            write32(dram, slot_addr + 4, 512);

            let next_idx = current_used_idx.wrapping_add(1);
            write16(dram, used_idx_addr, next_idx);

            self.last_avail_idx = self.last_avail_idx.wrapping_add(1);
            self.set_used_buffer_notification();
            // println!("  => 인터럽트 핀 세팅 완료!");
        } else {
            // [디버그 확인용] 메모리 쓰기가 아직 도달하지 않아 다음 틱까지 대기합니다.
            // let dbg_word = read32(dram, avail_idx_addr & !3);
            // println!("  => [대기 중] 메모리에 커널 인덱스가 도달할 때까지 대기... (Raw 32bit: 0x{:08X})", dbg_word);
        }
    }
}
