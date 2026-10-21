// src/hardware/soc.rs
pub mod clint;
pub mod cpu;
pub mod memory;
pub mod system_bus;
pub mod systolic;
pub mod types;

use clint::Clint;
use cpu::Cpu;
use memory::Dram;
use system_bus::mmio_device::MmioDevice;
use system_bus::{BusOwner, SystemBus};
use systolic::SystolicArray;

use clint::{CLINT_BASE, CLINT_SIZE};
use system_bus::{MMIO_BASE, MMIO_SIZE};

pub struct SoC {
    pub cpu: Cpu,                // 코어 (Bus를 소유하지 않음)
    pub systolic: SystolicArray, // 가속기
    pub clint: Clint,            // 하드웨어 타이머/인터럽트
    pub dram: Dram,              // 메인 메모리
    pub bus_owner: BusOwner,     // 현재 시스템 버스 점유자
    pub cycle: u64,
}

// SoC는 CPU, 가속기, 메모리 등 여러 하드웨어 부품을 통합한 시스템입니다.
impl SoC {
    pub fn new(mem_size: usize) -> Self {
        Self {
            cpu: Cpu::new(),
            systolic: SystolicArray::new(),
            clint: Clint::new(),
            dram: Dram::new(mem_size),
            bus_owner: BusOwner::None,
            cycle: 0,
        }
    }

    // 1 Cycle 틱
    pub fn tick(&mut self) {
        self.tick_with(false);
    }

    pub fn tick_with(&mut self, inject_nop: bool) {
        self.cycle += 1;
        self.dram.tick();
        self.bus_owner = BusOwner::None; // 매 사이클마다 버스 소유권 초기화

        // 가속기 DMA도 CPU와 같은 시스템 버스를 사용합니다.
        let mut empty_mmio: [(u32, u32, &mut dyn MmioDevice); 0] = [];
        let mut dma_bus = SystemBus::new(self.bus_owner, &mut self.dram, &mut empty_mmio);
        self.systolic.step(&mut dma_bus);
        self.bus_owner = dma_bus.owner;

        // CLINT 타이머 동작 및 인터럽트 발생 여부 확인
        let (sw_irq, timer_irq) = self.clint.tick();
        let ext_irq = false;
        self.cpu
            .csr
            .update_interrupt_pins(sw_irq, timer_irq, ext_irq);

        // 부품들의 참조를 모아 시스템 버스 인터페이스를 생성
        let mut mmio_devices: [(u32, u32, &mut dyn MmioDevice); 2] = [
            (MMIO_BASE, MMIO_SIZE, &mut self.systolic),
            (CLINT_BASE, CLINT_SIZE, &mut self.clint),
        ];

        // CPU 실행 (CPU가 버스/메모리에 접근할 수 있도록 컨텍스트를 묶어서 전달)
        // Rust의 Borrow Checker를 통과하기 위해 SoC의 필드들을 분리해서 참조로 넘깁니다.
        let mut sys_bus = SystemBus::new(self.bus_owner, &mut self.dram, &mut mmio_devices);
        self.cpu.pipeline_step(&mut sys_bus, inject_nop);
        self.bus_owner = sys_bus.owner;
    }
}
