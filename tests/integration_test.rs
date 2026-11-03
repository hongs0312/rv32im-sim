mod common;

use common::{load_program, run_cycles};

#[test]
fn pipeline_handles_forwarding_load_use_and_branch_flush() {
    let program = [
        0x00a00093, // addi x1, x0, 10
        0x01400113, // addi x2, x0, 20
        0x002081b3, // add  x3, x1, x2
        0x40118233, // sub  x4, x3, x1
        0x00402023, // sw   x4, 0(x0)
        0x00002283, // lw   x5, 0(x0)
        0x00128333, // add  x6, x5, x1
        0x00108663, // beq  x1, x1, 12
        0x3e700393, // flushed: addi x7, x0, 999
        0x06400413, // addi x8, x0, 100
        0x06400413, // branch target
    ];
    let mut hw = load_program(&program);

    run_cycles(&mut hw, 80);

    assert_eq!(hw.soc.cpu.core0.regs.read(1), 10);
    assert_eq!(hw.soc.cpu.core0.regs.read(2), 20);
    assert_eq!(hw.soc.cpu.core0.regs.read(3), 30);
    assert_eq!(hw.soc.cpu.core0.regs.read(4), 20);
    assert_eq!(hw.soc.cpu.core0.regs.read(5), 20);
    assert_eq!(hw.soc.cpu.core0.regs.read(6), 30);
    assert_eq!(hw.soc.cpu.core0.regs.read(7), 0);
    assert_eq!(hw.soc.cpu.core0.regs.read(8), 100);
}

#[test]
fn soc_tick_advances_shared_bus_and_cpu_together() {
    let mut hw = load_program(&[0x02a00093]); // addi x1, x0, 42

    run_cycles(&mut hw, 12);

    assert_eq!(hw.soc.cpu.core0.regs.read(1), 42);
    assert!(hw.soc.cycle >= 12);
}
