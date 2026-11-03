// use std::io::{Read, Write, Seek, SeekFrom};
use crate::hardware::mmio_device::MmioDevice;

use super::VirtioBlk;

impl MmioDevice for VirtioBlk {
    fn read(&mut self, offset: u32) -> Result<u32, String> {
        let val = match offset {
            0x000 => 0x74726976, // MagicValue: "virt"
            0x004 => 1,          // 💡 Version: 1 (Legacy 인터페이스는 반드시 1!)
            0x008 => 2,          // DeviceID: 2 (Block Device)
            0x00C => 0x554d4551, // VendorID: "QEMU"
            0x010 => self.host_features,
            0x034 => 32,             // QueueNumMax: 최대 32개 디스크립터 지원
            0x040 => self.queue_pfn, // QueuePFN
            0x060 => self.interrupt_status,
            0x070 => self.status,

            // Config 영역 (0x100 ~ 0x107): 디스크 용량(섹터 수, 64비트) 반환
            0x100 => (self.capacity & 0xFFFF_FFFF) as u32,
            0x104 => (self.capacity >> 32) as u32,

            _ => 0,
        };
        Ok(val)
    }

    fn write(&mut self, offset: u32, value: u32) -> Result<(), String> {
        match offset {
            0x020 => self.guest_features = value,
            0x028 => self.guest_page_size = value,
            0x030 => self.queue_sel = value,
            0x038 => self.queue_num = value,
            0x03C => self.queue_align = value,
            0x040 => self.queue_pfn = value,
            0x050 => {
                // 💡 QueueNotify (Doorbell): 커널이 디스크 I/O 요청을 큐에 넣었음!
                // MMIO write 안에서는 직접 DRAM을 건드리지 않고, DMA 요청 플래그만 켭니다.
                if value == 0 {
                    self.pending_notify = true;
                }
            }
            0x064 => {
                // InterruptACK
                self.interrupt_status &= !value;
            }
            0x070 => {
                // Status: 커널이 0을 쓰면 디바이스 리셋
                if value == 0 {
                    self.reset();
                } else {
                    self.status = value;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
