use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

// 💡 SoC 대신 최상위 메인보드인 Hardware를 임포트합니다.
use crate::RAM_SIZE;
use crate::hardware::Hardware;

pub fn single_cycle_simulation(mut hardware: Hardware, verbose: bool, max_steps: usize) {
    println!("[3/3] 싱글사이클 시뮬레이션 시작\n");
    println!("{:=^70}", " Simulation Running ");

    let mut cycle_count = 0;

    loop {
        if cycle_count >= max_steps {
            println!(
                "\n[경고] 최대 실행 클럭 수({})에 도달하여 종료합니다.",
                max_steps
            );
            break;
        }

        // 1. [안전장치] PC가 범위를 벗어났는지 확인 (core0 접근)
        if !hardware
            .soc
            .is_valid_mem_addr(hardware.soc.cpu.core0.pc, RAM_SIZE as u32)
        {
            println!(
                "\n[경고] PC(0x{:08X})가 유효한 메모리(또는 MMIO) 범위를 벗어났습니다. 비정상 종료를 감지하고 시뮬레이션을 중단합니다.",
                hardware.soc.cpu.core0.pc
            );
            break;
        }

        if verbose {
            println!(
                "[{:05} Cycle] PC: 0x{:08X} | sp: 0x{:08X} | a0: {} | s0: {} | s5: {}",
                cycle_count,
                hardware.soc.cpu.core0.pc,
                hardware.soc.cpu.core0.regs.read(2),
                hardware.soc.cpu.core0.regs.read(10),
                hardware.soc.cpu.core0.regs.read(8),
                hardware.soc.cpu.core0.regs.read(15)
            );
        }

        // 최상위 클럭 틱 전파
        hardware.tick_with(false);
        cycle_count += 1;

        // ecall fetch 시점에서 종료 조건 확인
        if hardware.soc.cpu.core0.if_id_reg.instruction == 0x00000073 {
            let exit_code = hardware.soc.cpu.core0.regs.read(10); // a0 (x10)
            println!("\n{:=^70}", " Simulation Finished ");
            println!(
                ">> Program exited gracefully with status code: {} (0x{:X})",
                exit_code, exit_code
            );
            println!(">> Total executed cycles: {} cycles", cycle_count);
            break;
        }

        // 충분한 사이클 동안 파이프라인 펌핑 (NOP 주입)
        for _ in 0..4 {
            hardware.tick_with(true);
            cycle_count += 1;
        }
    }
}

pub fn pipeline_simulation(mut hardware: Hardware, verbose: bool, max_steps: usize) {
    println!("[3/3] 파이프라인 시뮬레이션 시작\n");
    println!("{:=^70}", " Simulation Running ");

    let mut cycle_count = 0;

    loop {
        if cycle_count >= max_steps {
            println!("\n[경고] 최대 실행 클럭 수에 도달하여 종료합니다.");
            break;
        }

        // 1. ecall이 파이프라인 끝자락에 도달했는지 먼저 검사!
        if hardware.soc.cpu.core0.mem_wb_reg.control.is_ecall {
            break;
        }

        // 💡 외부 기기로 분리된 syscon 검사
        if hardware.syscon.is_halted {
            println!("\n{:=^70}", " System Power Off ");
            println!(">> Exit Code: {}", hardware.syscon.exit_code);

            println!(
                ">> [CSR Raw] mstatus: {:X?}, mie: {:X?}, mip: {:X?}",
                hardware
                    .soc
                    .cpu
                    .core0
                    .csr
                    .read(0x300, hardware.soc.cpu.core0.privilege_mode),
                hardware
                    .soc
                    .cpu
                    .core0
                    .csr
                    .read(0x304, hardware.soc.cpu.core0.privilege_mode),
                hardware
                    .soc
                    .cpu
                    .core0
                    .csr
                    .read(0x344, hardware.soc.cpu.core0.privilege_mode),
            );
            break;
        }

        // 2. [안전장치 1] PC 메모리 범위 이탈 확인
        if !hardware
            .soc
            .is_valid_mem_addr(hardware.soc.cpu.core0.pc, RAM_SIZE as u32)
        {
            println!(
                "\n[경고] PC(0x{:08X})가 유효한 메모리(또는 MMIO) 범위를 벗어났습니다. 비정상 종료를 감지하고 시뮬레이션을 중단합니다.",
                hardware.soc.cpu.core0.pc
            );
            break;
        }

        // 3. [안전장치 2] 뒤따라오는 쓰레기 명령어가 아닐 때만 메모리 에러 검출
        if hardware.soc.cpu.core0.ex_mem_reg.control.mem_read
            || hardware.soc.cpu.core0.ex_mem_reg.control.mem_write
        {
            let addr = hardware.soc.cpu.core0.ex_mem_reg.alu_result;

            if !hardware.soc.is_valid_mem_addr(addr, RAM_SIZE as u32) {
                println!("\n[경고] 잘못된 메모리 접근 감지 (주소: 0x{:08X}).", addr);
                println!(">> Total executed cycles: {} cycles", cycle_count);
                break;
            }
        }

        if verbose {
            println!(
                "[{:05} Cycle] IF_PC: 0x{:08X} | ID_PC: 0x{:08X} | EX_PC: 0x{:08X} | sp: 0x{:08X} | a0: {} | s0: {} | s5: {}",
                cycle_count,
                hardware.soc.cpu.core0.pc,
                hardware.soc.cpu.core0.if_id_reg.pc,
                hardware.soc.cpu.core0.id_ex_reg.pc,
                hardware.soc.cpu.core0.regs.read(2),
                hardware.soc.cpu.core0.regs.read(10),
                hardware.soc.cpu.core0.regs.read(8),
                hardware.soc.cpu.core0.regs.read(15)
            );
        }

        // 1클럭(사이클) 수행
        hardware.tick();

        cycle_count += 1;
    }
}

pub fn os_simulation(mut hardware: Hardware, verbose: bool, _max_steps: usize) {
    println!("[3/3] OS 파이프라인 시뮬레이션 시작\n");
    println!("{:=^70}", " OS Running ");

    let mut cycle_count = 0;

    enable_raw_mode().expect("Failed to enable raw mode");

    loop {
        // syscon 전원 제어 확인
        if hardware.syscon.is_halted {
            // 에뮬레이터 종료 시 터미널을 원래 상태로 복구합니다.
            disable_raw_mode().expect("Failed to disable raw mode");

            println!("\n{:=^70}", " System Power Off ");
            println!(">> Exit Code: {}", hardware.syscon.exit_code);

            println!(
                ">> [CSR Raw] mstatus: {:X?}, mie: {:X?}, mip: {:X?}",
                hardware
                    .soc
                    .cpu
                    .core0
                    .csr
                    .read(0x300, hardware.soc.cpu.core0.privilege_mode),
                hardware
                    .soc
                    .cpu
                    .core0
                    .csr
                    .read(0x304, hardware.soc.cpu.core0.privilege_mode),
                hardware
                    .soc
                    .cpu
                    .core0
                    .csr
                    .read(0x344, hardware.soc.cpu.core0.privilege_mode),
            );
            break;
        }

        if verbose {
            println!(
                "[{:05} Cycle] IF_PC: 0x{:08X} | ID_PC: 0x{:08X} | EX_PC: 0x{:08X} | sp: 0x{:08X} | a0: {} | s0: {} | s5: {}",
                cycle_count,
                hardware.soc.cpu.core0.pc,
                hardware.soc.cpu.core0.if_id_reg.pc,
                hardware.soc.cpu.core0.id_ex_reg.pc,
                hardware.soc.cpu.core0.regs.read(2),
                hardware.soc.cpu.core0.regs.read(10),
                hardware.soc.cpu.core0.regs.read(8),
                hardware.soc.cpu.core0.regs.read(15)
            );
        }

        hardware.tick();
        cycle_count += 1;
    }
}
