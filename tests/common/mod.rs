use rv32i_sim::hardware::Hardware;

pub const RAM_SIZE: usize = 16 * 1024 * 1024;
// 테스트용 더미 디스크 경로 지정 (VirtioBlk 초기화를 위해 필요)
const DUMMY_DISK_PATH: &str = "test_dummy.img";

pub fn load_program(program: &[u32]) -> Hardware {
    // 1. SoC 대신 최상위 Hardware 객체 생성
    let mut hw = Hardware::new(RAM_SIZE, DUMMY_DISK_PATH);

    // 2. 코어 PC 초기화
    hw.soc.cpu.core0.pc = 0x8000_0000; // DRAM_BASE

    // 3. 프로그램 메모리 적재
    for (index, instruction) in program.iter().copied().enumerate() {
        let address = (index * 4) as u32 + 0x8000_0000;
        hw.soc
            .dram
            .load_firmware(address, &instruction.to_le_bytes());
    }

    hw
}

pub fn run_cycles(hw: &mut Hardware, cycles: usize) {
    for _ in 0..cycles {
        // 4. soc.tick()이 아닌 Hardware의 tick()을 호출하여 모든 외부 장치 구동
        hw.tick();
    }
}
