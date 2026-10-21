use crate::hardware::soc::system_bus::mmio_device::MmioDevice;

use super::SystolicArray;

impl MmioDevice for SystolicArray {
    fn read(&self, offset: u32) -> Result<u32, String> {
        match offset {
            0x00 => Ok(self.status),
            0x20 => Ok(self.global_time),
            _ => Err(format!(
                "Invalid Systolic MMIO read offset: 0x{:02X}",
                offset
            )),
        }
    }

    fn write(&mut self, offset: u32, value: u32) -> Result<(), String> {
        match offset {
            0x04 => self.dma.addr_a = value,
            0x08 => self.dma.addr_b = value,
            0x0C => self.addr_c = value,
            0x10 => {
                if value == 1 {
                    self.start(self.dma.addr_a, self.dma.addr_b, self.addr_c);
                }
            }
            _ => {
                return Err(format!(
                    "Invalid Systolic MMIO write offset: 0x{:02X}",
                    offset
                ));
            }
        }
        Ok(())
    }
}
