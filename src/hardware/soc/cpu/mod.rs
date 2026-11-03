pub mod core;

use crate::hardware::soc::system_bus::SystemBus;

pub struct Cpu {
    pub core0: core::Core,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            core0: core::Core::new(),
        }
    }

    /// SoC로부터 인터럽트 신호를 받아 각 코어에 분배
    pub fn update_interrupt_pins(&mut self, sw_irq: bool, timer_irq: bool, ext_irq: bool) {
        // 나중에 듀얼코어가 되면 CLINT가 주는 timer_irq_0, timer_irq_1을
        // 각각 core0, core1로 찢어서 넣어주는 라우팅 로직이 여기에 들어갑니다.
        self.core0
            .csr
            .update_interrupt_pins(sw_irq, timer_irq, ext_irq);
    }

    pub fn tick(&mut self, mem_bus: &mut SystemBus, inject_nop: bool) {
        self.core0.pipeline_step(mem_bus, inject_nop);
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}
