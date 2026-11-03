use clap::Parser;
use std::path::Path;

use rv32i_sim::RAM_SIZE;
use rv32i_sim::{loader, logger, runner};

use rv32i_sim::hardware::Hardware;

#[derive(Parser, Debug)]
#[command(author, version, about = "RV32I C-Code Compiler & Pipelined Simulator")]
struct Args {
    #[arg(short, long, default_value = "files/build_baremetal.sh")]
    source: String,

    #[arg(long, default_value = "files/mini_sbi.s")]
    sbi_source: String, // SBI 어셈블리 소스 경로 추가

    #[arg(long, default_value = "files/baremetal.bin")]
    bin_source: String, // 바이너리 소스 여부 플래그 추가

    #[arg(short, long, default_value_t = false)]
    pipeline: bool,

    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    #[arg(short, long, default_value_t = 100000)]
    max_steps: usize,

    #[arg(short, long, default_value_t = 0)]
    cycles: u64,

    #[arg(short, long, default_value_t = false)]
    tui: bool,

    #[arg(short, long, value_parser = parse_hex, default_value_t = 0)]
    breakpoint: u32,

    #[arg(short, long, default_value_t = false)]
    os_mode: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if !Path::new(&args.source).exists() {
        eprintln!("오류: '{}' 파일을 찾을 수 없습니다.", args.source);
        return Ok(());
    }

    let sbi_program = loader::compile_sbi(&args.sbi_source);

    let mut hardware = Hardware::new(RAM_SIZE, "./files/1000_os/disk.tar");

    // 1. 모드에 따른 SoC 초기화 (soc를 mut로 선언!)
    if args.os_mode {
        println!("[1/3] OS 및 펌웨어 로드 준비");
        let os_program = loader::compile_and_extract_os("files/1000_os", "files/1000_os/kernel.ld");
        loader::setup_hardware_with_sbi(&mut hardware, &sbi_program, &os_program);
    } else {
        println!("[1/3] 베어메탈 프로그램 로드 준비");
        let baremetal_program = loader::compile_and_extract(&args.source, &args.bin_source);
        loader::setup_hardware_with_sbi(&mut hardware, &sbi_program, &baremetal_program);
    };

    // 2. 브레이크포인트(Fast-forward) 공통 처리
    // args.breakpoint 가 0이 아닐 때만 작동하도록 분기 (또는 Option<u32> 처리)
    if args.breakpoint != 0 {
        println!(
            "[2/3] 브레이크포인트(0x{:08X})까지 고속 실행 중...",
            args.breakpoint
        );
        loop {
            // 주의: 실제 PC 변수 경로(hardware.soc.cpu.core0.pc 등)에 맞게 수정하세요.
            if hardware.soc.cpu.core0.pc == args.breakpoint || hardware.syscon.is_halted {
                println!("목표 PC 0x{:08X} 도달 완료!", hardware.soc.cpu.core0.pc);
                break;
            }
            hardware.tick();
        }
    } else if args.cycles > 0 {
        println!("[2/3] {} 사이클 동안 고속 실행 중...", args.cycles);

        for _ in 0..args.cycles {
            if hardware.syscon.is_halted {
                println!(
                    "프로그램이 종료되었습니다. (PC: 0x{:08X})",
                    hardware.soc.cpu.core0.pc
                );
                break;
            }
            hardware.tick();
        }
    } else {
        println!("[2/3] 브레이크포인트 미설정 (초기 상태에서 대기)");
    }

    // 3. Runner 실행 분기
    if args.os_mode {
        // OS 모드 전용 러너 실행
        runner::os_simulation(hardware, args.verbose, args.max_steps);
    } else {
        if args.tui {
            // 💡 여기서 전역 TUI 플래그를 켜줍니다!
            logger::IS_TUI_MODE.store(true, std::sync::atomic::Ordering::Relaxed);

            println!("[3/3] TUI 시뮬레이터 시작\n");
            // TUI는 이제 백그라운드 로직 없이 화면만 그리면 됨!
            rv32i_sim::tui::app::run_tui(hardware)?;
        } else if args.pipeline {
            runner::pipeline_simulation(hardware, args.verbose, args.max_steps);
        } else {
            runner::single_cycle_simulation(hardware, args.verbose, args.max_steps);
        }
    }

    Ok(())
}

// 1. 16진수 문자열을 u32로 파싱하는 커스텀 함수 생성
fn parse_hex(src: &str) -> Result<u32, std::num::ParseIntError> {
    // "0x"나 "0X"로 시작하면 잘라내고, 16진수로 변환
    let src = src.trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(src, 16)
}
