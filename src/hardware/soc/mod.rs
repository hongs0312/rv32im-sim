pub mod cpu;
pub mod memory;
pub mod system_bus;
pub mod systolic;

// src/hardware/soc.rs
use cpu::Cpu;
use memory::Dram;
use system_bus::{BusOwner, BusState, SystemBus};
use systolic::SystolicArray;

pub struct SoC {
    pub cpu: Cpu,                // 코어 (Bus를 소유하지 않음)
    pub systolic: SystolicArray, // 가속기
    pub dram: Dram,              // 메인 메모리
    pub bus_state: BusState,     // 시스템 버스 중재 상태
    pub bus_owner: BusOwner,     // 현재 시스템 버스 점유자
    pub cycle: u64,
}

// SoC는 CPU, 가속기, 메모리 등 여러 하드웨어 부품을 통합한 시스템입니다.
impl SoC {
    pub fn new(mem_size: usize) -> Self {
        Self {
            cpu: Cpu::new(),
            systolic: SystolicArray::new(),
            dram: Dram::new(mem_size),
            bus_state: BusState::Ready,
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

        // 1. 가속기 DMA도 CPU와 같은 시스템 버스를 사용합니다.
        let mut dma_bus = SystemBus::memory(self.bus_state, self.bus_owner, &mut self.dram);
        self.systolic.step(&mut dma_bus);
        self.bus_state = dma_bus.state;
        self.bus_owner = dma_bus.owner;

        // 부품들의 참조를 모아 시스템 버스 인터페이스를 생성
        let mut sys_bus = SystemBus::with_systolic(
            self.bus_state,
            self.bus_owner,
            &mut self.dram,
            &mut self.systolic,
        );

        // 2. CPU 실행 (CPU가 버스/메모리에 접근할 수 있도록 컨텍스트를 묶어서 전달)
        // Rust의 Borrow Checker를 통과하기 위해 SoC의 필드들을 분리해서 참조로 넘깁니다.
        self.cpu.pipeline_step(&mut sys_bus, inject_nop);
        self.bus_state = sys_bus.state;
        self.bus_owner = sys_bus.owner;
    }
}
