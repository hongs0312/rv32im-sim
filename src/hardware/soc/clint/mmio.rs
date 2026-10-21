/*
    mmio.rs
    mmio.rs는 CLINT(클록 및 인터럽트 제어기) 장치의 메모리 맵 I/O(MMIO) 인터페이스를 구현하는 모듈입니다.
    이 모듈은 CLINT 장치가 MMIO를 통해 CPU와 상호작용할 수 있도록 read 및 write 메서드를 제공합니다.
    CLINT 장치는 RISC-V 아키텍처에서 타이머 및 소프트웨어 인터럽트를 관리하는 하드웨어 장치입니다.
*/

use crate::hardware::soc::system_bus::mmio_device::MmioDevice;

use super::Clint;
use super::{
    MSIP_OFFSET, MTIME_HI_OFFSET, MTIME_LO_OFFSET, MTIMECMP_HI_OFFSET, MTIMECMP_LO_OFFSET,
};

impl MmioDevice for Clint {
    fn read(&self, offset: u32) -> Result<u32, String> {
        match offset {
            MSIP_OFFSET => Ok(self.msip & 1),
            MTIMECMP_LO_OFFSET => Ok(self.mtimecmp as u32),
            MTIMECMP_HI_OFFSET => Ok((self.mtimecmp >> 32) as u32),
            MTIME_LO_OFFSET => Ok(self.mtime as u32),
            MTIME_HI_OFFSET => Ok((self.mtime >> 32) as u32),
            _ => Err(format!("Invalid CLINT read offset: 0x{:04X}", offset)),
        }
    }

    fn write(&mut self, offset: u32, value: u32) -> Result<(), String> {
        match offset {
            MSIP_OFFSET => self.msip = value & 1,
            MTIMECMP_LO_OFFSET => {
                self.mtimecmp = (self.mtimecmp & !0xFFFF_FFFF) | (value as u64);
            }
            MTIMECMP_HI_OFFSET => {
                self.mtimecmp = (self.mtimecmp & 0xFFFF_FFFF) | ((value as u64) << 32);
            }
            MTIME_LO_OFFSET => {
                self.mtime = (self.mtime & !0xFFFF_FFFF) | (value as u64);
            }
            MTIME_HI_OFFSET => {
                self.mtime = (self.mtime & 0xFFFF_FFFF) | ((value as u64) << 32);
            }
            _ => return Err(format!("Invalid CLINT write offset: 0x{:04X}", offset)),
        }
        Ok(())
    }
}
