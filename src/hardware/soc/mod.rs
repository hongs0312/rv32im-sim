// src/hardware/soc/mod.rs
pub mod cpu;
pub mod memory;
pub mod system_bus;
pub mod systolic;
pub mod types;

use cpu::Cpu;
use memory::{DRAM_BASE, Dram};
use system_bus::{BusOwner, SystemBus};
use systolic::{SYSTOLIC_BASE, SYSTOLIC_SIZE, SystolicArray};

use crate::hardware::IrqLines;
use crate::hardware::clint::{CLINT_BASE, CLINT_SIZE};
use crate::hardware::mmio_device::MmioDevice;
use crate::hardware::syscon::{SYSCON_BASE, SYSCON_SIZE};
use crate::hardware::uart::{UART_BASE, UART_SIZE};
use crate::hardware::virtio_blk::{VIRTIO_BASE, VIRTIO_SIZE};

pub struct SoC {
    pub cpu: Cpu,
    pub dram: Dram,
    pub bus_owner: BusOwner,
    pub systolic: SystolicArray,
    pub cycle: u64,
}

impl SoC {
    pub fn new(mem_size: usize) -> Self {
        Self {
            cpu: Cpu::new(),
            dram: Dram::new(mem_size),
            bus_owner: BusOwner::None,
            systolic: SystolicArray::new(),
            cycle: 0,
        }
    }

    pub fn is_valid_mem_addr(&self, addr: u32, ram_size: u32) -> bool {
        let in_ram = addr >= DRAM_BASE && addr < DRAM_BASE + ram_size;
        let in_clint = (CLINT_BASE..CLINT_BASE + CLINT_SIZE).contains(&addr);
        let in_uart = (UART_BASE..UART_BASE + UART_SIZE).contains(&addr);
        let in_systolic = (SYSTOLIC_BASE..SYSTOLIC_BASE + SYSTOLIC_SIZE).contains(&addr);
        let in_syscon = (SYSCON_BASE..SYSCON_BASE + SYSCON_SIZE).contains(&addr);
        let in_virtio = (VIRTIO_BASE..VIRTIO_BASE + VIRTIO_SIZE).contains(&addr);

        in_ram || in_clint || in_uart || in_systolic || in_syscon || in_virtio
    }

    // 계층 2: SoC -> Cpu (-> Core) 틱 전파
    pub fn tick_with(
        &mut self,
        inject_nop: bool,
        ext_mmio: &mut [(u32, u32, &mut dyn MmioDevice)],
        irqs: IrqLines,
    ) {
        self.cycle += 1;
        self.dram.tick();
        self.bus_owner = BusOwner::None;

        // 1. DMA 버스 마스터 실행 (Systolic Array 및 향후 통합 DMA 엔진)
        {
            let mut empty_in: [(u32, u32, &mut dyn MmioDevice); 0] = [];
            let mut empty_ext: [(u32, u32, &mut dyn MmioDevice); 0] = [];
            let mut dma_bus = SystemBus::new(
                self.bus_owner,
                &mut self.dram,
                &mut empty_in,
                &mut empty_ext,
            );
            self.systolic.step(&mut dma_bus);
            self.bus_owner = dma_bus.owner;
        }

        // 2. 인터럽트 핀 전달
        self.cpu
            .update_interrupt_pins(irqs.sw_irq, irqs.timer_irq, irqs.ext_irq);

        // 3. 칩 내부 MMIO 슬라이스 구성
        let mut int_mmio: [(u32, u32, &mut dyn MmioDevice); 1] =
            [(SYSTOLIC_BASE, SYSTOLIC_SIZE, &mut self.systolic)];

        // 4. CPU 버스 생성 및 Cpu::tick 실행
        let mut sys_bus = SystemBus::new(self.bus_owner, &mut self.dram, &mut int_mmio, ext_mmio);
        self.cpu.tick(&mut sys_bus, inject_nop);

        self.bus_owner = sys_bus.owner;
    }

    pub fn get_cycle(&self) -> u64 {
        self.cycle
    }
}
