mod common;

use common::{load_program, run_cycles};

#[test]
fn pipeline_step_is_driven_by_soc_clock() {
    let program = [
        0x00a00093, // addi x1, x0, 10
        0x01400113, // addi x2, x0, 20
        0x002081b3, // add  x3, x1, x2
    ];
    let mut hw = load_program(&program);

    run_cycles(&mut hw, 60);

    assert_eq!(hw.soc.cpu.core0.regs.read(1), 10);
    assert_eq!(hw.soc.cpu.core0.regs.read(2), 20);
    assert_eq!(hw.soc.cpu.core0.regs.read(3), 30);
}

#[test]
fn store_and_load_use_soc_dram_through_the_bus() {
    let program = [
        0x80001537, // lui  x10, 0x80001 → 0x8000_1000
        0x02a00093, // addi x1, x0, 42
        0x00152023, // sw   x1, 0(x10)
        0x00052103, // lw   x2, 0(x10)
        0x0000006f, // jal  x0, 0
    ];

    let mut hw = common::load_program(&program);

    common::run_cycles(&mut hw, 80);

    let regs = &hw.soc.cpu.core0.regs;

    assert_eq!(regs.read(10), 0x8000_1000);
    assert_eq!(regs.read(1), 42);
    assert_eq!(regs.read(2), 42);
}
