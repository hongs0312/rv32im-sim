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
        0x10000093, // addi x1, x0, 0x100
        0x02a00113, // addi x2, x0, 42
        0x0020a023, // sw x2, 0(x1)
        0x50000093, // addi x1, x0, 0x500 (same cache index, new tag)
        0x0000a183, // lw x3, 0(x1), evicts the dirty 0x100 line
    ];
    let mut hw = load_program(&program);

    run_cycles(&mut hw, 140);

    assert_eq!(hw.soc.dram.load32(0x100), 42);
    assert_eq!(hw.soc.cpu.core0.regs.read(3), 0);
}
