use crate::hardware::mmio_device::MmioDevice;

use super::Syscon;

impl MmioDevice for Syscon {
    fn read(&mut self, offset: u32) -> Result<u32, String> {
        if offset != 0 {
            return Err(format!("Invalid read offset for Syscon: 0x{:08X}", offset));
        }

        Ok(self.exit_code)
    }

    fn write(&mut self, offset: u32, value: u32) -> Result<(), String> {
        if offset != 0 {
            return Err(format!("Invalid write offset for Syscon: 0x{:08X}", offset));
        }

        self.is_halted = true;

        self.exit_code = match value {
            0x5555 => 0, // 정상 종료
            _ => 1,      // 비정상 종료
        };
        Ok(())
    }
}
