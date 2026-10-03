use clap::Parser;
use rv32i_sim::hardware::soc::SoC;
use std::{fs, path::Path, process::Command};

const RAM_SIZE: usize = 16 * 1024 * 1024; // 16MB

#[derive(Parser, Debug)]
#[command(author, version, about = "RV32I C-Code Compiler & Pipelined Simulator")]
struct Args {
    #[arg(short, long, default_value = "files/main.c")]
    source: String,

    #[arg(short, long, default_value_t = false)]
    pipeline: bool,

    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    #[arg(short, long, default_value_t = 100000)]
    max_steps: usize,

    #[arg(short, long, default_value_t = false)]
    tui: bool,
}

fn compile_and_extract(c_path: &str) -> Vec<u32> {
    println!("[1/3] C 소스 파일 컴파일 중: {}", c_path);

    let elf_path = "temp.elf";
    let bin_path = "temp.bin";

    // GCC 컴파일 수행
    let gcc_status = Command::new("riscv64-unknown-elf-gcc")
        .args([
            "-O2",
            "-nostdlib",
            "-mabi=ilp32",
            "-march=rv32im",
            "-mno-relax",
            "-T",
            "files/linker.ld",
            "files/entry.s",
            c_path,
            "-o",
            elf_path,
        ])
        .status()
        .expect("riscv64-unknown-elf-gcc 실행 실패. 환경변수 PATH를 확인하세요.");

    assert!(gcc_status.success(), "C 코드 컴파일 실패");

    // 바이너리 추출
    let objcopy_status = Command::new("riscv64-unknown-elf-objcopy")
        .args(["-O", "binary", elf_path, bin_path])
        .status()
        .expect("objcopy 실행 실패");

    assert!(objcopy_status.success(), "바이너리 추출 실패");

    // 생성된 바이너리 파일 읽기
    let binary_bytes = fs::read(bin_path).expect("바이너리 파일을 읽을 수 없습니다.");

    // 임시 파일 삭제
    let _ = fs::remove_file(elf_path);
    let _ = fs::remove_file(bin_path);

    // u8 바이트 배열을 u32 명령어 배열로 변환
    binary_bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

fn setup_cpu(program: &[u32]) -> SoC {
    println!("[2/3] CPU 및 16MB DRAM 초기화 중...");
    let mut soc = SoC::new(RAM_SIZE);

    for (i, &inst) in program.iter().enumerate() {
        let bytes = inst.to_le_bytes();
        let addr = i * 4;
        soc.dram.dram[addr] = bytes[0];
        soc.dram.dram[addr + 1] = bytes[1];
        soc.dram.dram[addr + 2] = bytes[2];
        soc.dram.dram[addr + 3] = bytes[3];
    }

    soc
}

fn run_single_cycle_simulation(mut soc: SoC, verbose: bool, max_steps: usize) {
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

        // 1. [안전장치] PC가 범위를 벗어났는지 확인
        if soc.cpu.pc >= RAM_SIZE as u32 {
            println!(
                "\n[경고] PC(0x{:08X})가 메모리 범위를 벗어났습니다. 비정상 종료를 감지하고 시뮬레이션을 중단합니다.",
                soc.cpu.pc
            );
            break;
        }

        if verbose {
            println!(
                "[{:05} Cycle] PC: 0x{:08X} | sp: 0x{:08X} | a0: {} | s0: {} | s5: {}",
                cycle_count,
                soc.cpu.pc,
                soc.cpu.regs.read(2),
                soc.cpu.regs.read(10),
                soc.cpu.regs.read(8),
                soc.cpu.regs.read(15)
            );
        }

        soc.tick_with(false);
        cycle_count += 1;

        // ecall fetch 시점에서 종료 조건 확인
        if soc.cpu.if_id_reg.instruction == 0x00000073 {
            let exit_code = soc.cpu.regs.read(10); // a0 (x10)
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
            soc.tick_with(true);
            cycle_count += 1;
        }
    }
}

fn run_pipeline_simulation(mut soc: SoC, verbose: bool, max_steps: usize) {
    println!("[3/3] 파이프라인 시뮬레이션 시작\n");
    println!("{:=^70}", " Simulation Running ");

    let mut cycle_count = 0;

    loop {
        if cycle_count >= max_steps {
            println!("\n[경고] 최대 실행 클럭 수에 도달하여 종료합니다.");
            break;
        }

        // 1. ecall이 파이프라인 끝자락에 도달했는지 먼저 검사!
        // WB 단계나 MEM 단계에 ecall이 있다면 정상 종료 절차를 밟음
        if soc.cpu.mem_wb_reg.control.is_ecall {
            let exit_code = soc.cpu.regs.read(10); // a0 (x10)
            let cpu_cycles = soc.cpu.regs.read(11); // a1 (x11) - CPU 연산 사이클
            let sys_cycles = soc.cpu.regs.read(12); // a2 (x12) - 가속기 연산 사이클

            println!("\n{:=^70}", " Simulation Finished ");
            println!(
                ">> Program exited gracefully with status code: {} (0x{:X})",
                exit_code, exit_code
            );
            println!(
                ">> Total executed cycles (Simulator): {} cycles\n",
                cycle_count
            );

            println!("{:-^70}", " 벤치마크 결과 (16x16 행렬 곱셈) ");
            println!(" - 순수 CPU (RV32IM) 소요 클럭  : {} cycles", cpu_cycles);
            println!(" - 시스톨릭 어레이 가속 소요 클럭: {} cycles", sys_cycles);

            if sys_cycles > 0 {
                println!(
                    " => 성능 향상(Speedup)         : 약 {} 배 빠름!",
                    cpu_cycles / sys_cycles
                );
            }
            println!("{:-^70}", "");

            break;
        }

        // 2. [안전장치 1] PC 메모리 범위 이탈 확인
        if soc.cpu.pc >= RAM_SIZE as u32 {
            println!(
                "\n[경고] PC(0x{:08X})가 메모리 범위를 벗어났습니다.",
                soc.cpu.pc
            );
            break;
        }

        // 3. [안전장치 2] 뒤따라오는 쓰레기 명령어가 아닐 때만 메모리 에러 검출
        if soc.cpu.ex_mem_reg.control.mem_read || soc.cpu.ex_mem_reg.control.mem_write {
            let addr = soc.cpu.ex_mem_reg.alu_result;

            // 물리 RAM 범위를 벗어났으면서, 동시에 MMIO 대역(0x8000_0000 이상)도 아닌 경우에만 에러!
            if addr >= RAM_SIZE as u32 && addr < 0x8000_0000 {
                println!("\n[경고] 잘못된 메모리 접근 감지 (주소: 0x{:08X}).", addr);
                println!(">> Total executed cycles: {} cycles", cycle_count);
                break;
            }
        }

        if verbose {
            println!(
                "[{:05} Cycle] IF_PC: 0x{:08X} | ID_PC: 0x{:08X} | EX_PC: 0x{:08X} | sp: 0x{:08X} | a0: {} | s0: {} | s5: {}",
                cycle_count,
                soc.cpu.pc,
                soc.cpu.if_id_reg.pc,
                soc.cpu.id_ex_reg.pc,
                soc.cpu.regs.read(2),
                soc.cpu.regs.read(10),
                soc.cpu.regs.read(8),
                soc.cpu.regs.read(15)
            );
        }

        // 1클럭(사이클) 수행
        soc.tick();

        cycle_count += 1;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if !Path::new(&args.source).exists() {
        eprintln!("오류: '{}' 파일을 찾을 수 없습니다.", args.source);
        return Ok(());
    }

    let program = compile_and_extract(&args.source);
    let soc = setup_cpu(&program);

    if args.tui {
        println!("[3/3] TUI 시뮬레이터 시작\n");
        rv32i_sim::tui::app::run_tui(soc)?;
    } else if args.pipeline {
        run_pipeline_simulation(soc, args.verbose, args.max_steps);
    } else {
        run_single_cycle_simulation(soc, args.verbose, args.max_steps);
    }

    Ok(())
}
