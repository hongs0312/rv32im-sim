use crate::hardware::Hardware;
use std::{fs, path::Path, process::Command};

/// 외부 쉘 스크립트를 실행하여 베어메탈(또는 벤치마크) 프로그램을 컴파일하고 바이너리를 추출합니다.
/// * `script_path`: 실행할 쉘 스크립트 경로 (예: "files/benchmark/build.sh")
/// * `output_bin`: 스크립트 실행 후 생성될 결과물 바이너리 경로 (예: "files/benchmark/bench.bin")
pub fn compile_and_extract(script_path: &str, output_bin: &str) -> Vec<u32> {
    println!(
        "[1/3] 외부 스크립트를 통해 베어메탈 빌드 시작: {}",
        script_path
    );

    // 스크립트가 위치한 디렉토리를 작업 디렉토리로 설정
    let script_dir = Path::new(script_path).parent().unwrap_or(Path::new("."));
    let script_name = Path::new(script_path).file_name().unwrap_or_default();

    let build_output = Command::new("bash")
        .arg(script_name)
        .current_dir(script_dir)
        .output()
        .expect("빌드 스크립트 실행 실패");

    if !build_output.status.success() {
        eprintln!(
            "베어메탈 빌드 스크립트 에러:\n{}",
            String::from_utf8_lossy(&build_output.stderr)
        );
        std::process::exit(1);
    }

    // 스크립트의 정상 출력 로그를 화면에 표시
    print!("{}", String::from_utf8_lossy(&build_output.stdout));

    // 생성된 바이너리 경로 확인 및 읽기
    let bin_bytes = fs::read(output_bin)
        .unwrap_or_else(|_| panic!("추출된 바이너리 파일 읽기 실패: {}", output_bin));

    // u8 배열을 시뮬레이터 규격인 u32 배열로 변환
    bin_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

pub fn compile_and_extract_os(src_dir: &str, _linker_script: &str) -> Vec<u32> {
    println!(
        "  -> 외부 쉘 스크립트를 통해 OS 빌드 시작: {}/build.sh",
        src_dir
    );

    // 1. OS 폴더 내부의 build.sh 실행
    let build_output = Command::new("bash")
        .arg("build.sh")
        .current_dir(src_dir)
        .output()
        .expect("빌드 스크립트(build.sh) 실행 실패");

    if !build_output.status.success() {
        eprintln!(
            "OS 빌드 스크립트 에러:\n{}",
            String::from_utf8_lossy(&build_output.stderr)
        );
        std::process::exit(1);
    }

    // 스크립트의 정상 출력 로그를 화면에 표시
    print!("{}", String::from_utf8_lossy(&build_output.stdout));

    // 2. 생성된 kernel.bin 경로 확인 및 읽기
    let bin_path = format!("{}/kernel.bin", src_dir);
    let bin_bytes = fs::read(&bin_path)
        .unwrap_or_else(|_| panic!("추출된 kernel.bin 파일 읽기 실패: {}", bin_path));

    // 3. u8 배열을 시뮬레이터 규격인 u32 배열로 변환
    bin_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
    // .chunks(4)
    // .map(|chunk| {
    //     let mut buf = [0u8; 4];
    //     buf[..chunk.len()].copy_from_slice(chunk);
    //     u32::from_le_bytes(buf)
    // })
    // .collect()
}

/// SBI 어셈블리 실시간 컴파일 (OS 부팅 용)
pub fn compile_sbi(s_path: &str) -> Vec<u32> {
    println!("  -> SBI 펌웨어 실시간 어셈블 중: {}", s_path);
    let elf_path = "temp_sbi.elf";
    let bin_path = "temp_sbi.bin";

    let gcc_status = Command::new("riscv64-unknown-elf-gcc")
        .args([
            "-nostdlib",
            "-mabi=ilp32",
            "-march=rv32im",
            "-Wl,-Ttext=0x80000000", // SBI 시작 주소 강제 지정
            s_path,
            "-o",
            elf_path,
        ])
        .status()
        .expect("gcc 실행 실패");
    assert!(gcc_status.success(), "SBI 어셈블리 컴파일 실패");

    let objcopy_status = Command::new("riscv64-unknown-elf-objcopy")
        .args(["-O", "binary", elf_path, bin_path])
        .status()
        .expect("objcopy 실행 실패");
    assert!(objcopy_status.success(), "SBI 바이너리 추출 실패");

    let binary_bytes = fs::read(bin_path).expect("SBI 바이너리 읽기 실패");
    // let _ = fs::remove_file(elf_path);
    // let _ = fs::remove_file(bin_path);

    binary_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

/// 이미 빌드된 바이너리 읽기 (OS 커널 용)
pub fn load_binary(bin_path: &str) -> Vec<u32> {
    println!("  -> OS 바이너리 파일 로드 중: {}", bin_path);
    let binary_bytes =
        fs::read(bin_path).unwrap_or_else(|_| panic!("'{}' 파일을 읽을 수 없습니다.", bin_path));

    binary_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

/// Bare-metal 단일 프로그램용 하드웨어 메모리 적재
pub fn setup_hardware_for_baremetal(hardware: &mut Hardware, program: &[u32]) {
    println!("[2/3] DRAM에 베어메탈 단일 프로그램 적재 중...");

    let base_addr = 0x8000_0000;
    println!("  -> 프로그램 적재 중 (시작 주소: 0x{:08X})", base_addr);

    for (i, &inst) in program.iter().enumerate() {
        hardware
            .soc
            .dram
            .load_firmware(base_addr + (i * 4) as u32, &inst.to_le_bytes());
    }

    // 0번 코어의 초기 PC를 0x8000_0000으로 세팅
    hardware.soc.cpu.core0.pc = base_addr;
}

/// SBI를 포함한 하드웨어 메모리 적재 (SBI + OS/베어메탈 동시 적재)
pub fn setup_hardware_with_sbi(hardware: &mut Hardware, sbi_program: &[u32], os_program: &[u32]) {
    println!("[2/3] CPU 및 DRAM 초기화...");

    let sbi_base_addr = 0x8000_0000;
    println!(
        "  -> SBI 펌웨어 적재 중 (시작 주소: 0x{:08X})",
        sbi_base_addr
    );
    for (i, &inst) in sbi_program.iter().enumerate() {
        hardware
            .soc
            .dram
            .load_firmware(sbi_base_addr + (i * 4) as u32, &inst.to_le_bytes());
    }

    let os_base_addr = 0x8020_0000;
    println!("  -> 프로그램 적재 중 (시작 주소: 0x{:08X})", os_base_addr);
    for (i, &inst) in os_program.iter().enumerate() {
        hardware
            .soc
            .dram
            .load_firmware(os_base_addr + (i * 4) as u32, &inst.to_le_bytes());
    }

    // 초기 PC를 펌웨어 위치로 세팅
    hardware.soc.cpu.core0.pc = sbi_base_addr;
}
