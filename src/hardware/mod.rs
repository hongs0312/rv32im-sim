// src/hardware/mod.rs
pub mod clint;
pub mod mmio_device;
pub mod soc;
pub mod syscon;
pub mod uart;
pub mod virtio_blk;

use clint::{CLINT_BASE, CLINT_SIZE, Clint};
use mmio_device::MmioDevice;
use soc::SoC;
use syscon::{SYSCON_BASE, SYSCON_SIZE, Syscon};
use uart::{UART_BASE, UART_SIZE, Uart};
use virtio_blk::{VIRTIO_BASE, VIRTIO_SIZE, VirtioBlk};

// 칩 외부에서 SoC 핀으로 들어가는 인터럽트 신호
#[derive(Clone, Copy, Default)]
pub struct IrqLines {
    pub sw_irq: bool,
    pub timer_irq: bool,
    pub ext_irq: bool,
}

pub struct Hardware {
    pub soc: SoC,
    pub clint: Clint,
    pub uart: Uart,
    pub syscon: Syscon,
    pub virtio_blk: VirtioBlk,
}

impl Hardware {
    pub fn new(mem_size: usize, disk_path: &str) -> Self {
        Self {
            soc: SoC::new(mem_size),
            clint: Clint::new(),
            uart: Uart::new(),
            syscon: Syscon::new(),
            virtio_blk: VirtioBlk::new(disk_path),
        }
    }

    pub fn tick(&mut self) {
        self.tick_with(false);
    }

    // 계층 1: Hardware -> SoC 틱 전파
    pub fn tick_with(&mut self, inject_nop: bool) {
        // 1. 외부 타이머(CLINT) 진행 및 인터럽트 수집
        let (sw_irq, timer_irq) = self.clint.tick();

        self.virtio_blk.process_sync_dma(&mut self.soc.dram);

        let irqs = IrqLines {
            sw_irq,
            timer_irq,
            ext_irq: self.virtio_blk.is_interrupting(), // VirtIO 인터럽트 핀 연결 준비
        };

        // 2. 외부 MMIO 슬라이스 구성 (CPU가 MMIO 레지스터를 읽고 쓸 때 사용)
        let mut ext_mmio: [(u32, u32, &mut dyn MmioDevice); 4] = [
            (CLINT_BASE, CLINT_SIZE, &mut self.clint),
            (UART_BASE, UART_SIZE, &mut self.uart),
            (SYSCON_BASE, SYSCON_SIZE, &mut self.syscon),
            (VIRTIO_BASE, VIRTIO_SIZE, &mut self.virtio_blk),
        ];

        // 3. SoC 틱 호출
        self.soc.tick_with(inject_nop, &mut ext_mmio, irqs);
    }
}
