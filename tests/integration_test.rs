mod common;

use common::{load_program, run_cycles};

#[test]
fn pipeline_handles_forwarding_load_use_and_branch_flush() {
    let program = [
        0x80001537, // lui  x10, 0x80001 → 데이터 주소 0x8000_1000
        0x00a00093, // addi x1, x0, 10
        0x01400113, // addi x2, x0, 20
        0x002081b3, // add  x3, x1, x2
        0x40118233, // sub  x4, x3, x1
        0x00452023, // sw   x4, 0(x10)
        0x00052283, // lw   x5, 0(x10)
        0x00128333, // add  x6, x5, x1
        0x00108663, // beq  x1, x1, 12
        0x3e700393, // 취소 대상: addi x7, x0, 999
        0x06300413, // 취소 대상: addi x8, x0, 99
        0x06400413, // 분기 목적지: addi x8, x0, 100
        0x0000006f, // jal x0, 0 → 종료 위치 유지
    ];

    let mut hw = load_program(&program);

    run_cycles(&mut hw, 80);

    let regs = &hw.soc.cpu.core0.regs;

    assert_eq!(regs.read(10), 0x8000_1000);

    assert_eq!(regs.read(1), 10);
    assert_eq!(regs.read(2), 20);

    // 산술 명령 간 포워딩.
    assert_eq!(regs.read(3), 30);
    assert_eq!(regs.read(4), 20);

    // Store → load 및 load-use 처리.
    assert_eq!(regs.read(5), 20);
    assert_eq!(regs.read(6), 30);

    // 잘못된 경로의 명령은 취소되어야 한다.
    assert_eq!(regs.read(7), 0);

    // 올바른 분기 목적지 실행.
    assert_eq!(regs.read(8), 100);
}

#[test]
fn soc_tick_advances_shared_bus_and_cpu_together() {
    let mut hw = load_program(&[
        0x02a00093, // addi x1, x0, 42
        0x0000006f, // jal x0, 0
    ]);

    run_cycles(&mut hw, 12);

    assert_eq!(hw.soc.cpu.core0.regs.read(1), 42);
    assert!(hw.soc.cycle >= 12);
}
