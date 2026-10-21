/*
    mmio_device.rs
    mmio_device.rs는 MMIO 장치의 공통 인터페이스를 정의하는 모듈입니다.
    MMIO 장치는 메모리 맵 I/O를 통해 CPU와 상호작용하는 하드웨어 장치를 나타냅니다. 이 모듈은 MMIO 장치가 구현해야 하는 공통 메서드를 정의합니다.
*/

pub trait MmioDevice {
    fn read(&self, offset: u32) -> Result<u32, String>;
    fn write(&mut self, offset: u32, value: u32) -> Result<(), String>;
}
