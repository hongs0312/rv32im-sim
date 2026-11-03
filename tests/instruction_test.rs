mod common;

use common::{load_program, run_cycles};

#[test]
fn arithmetic_and_logic_instructions_execute() {
    let program = [
        0x00a00093, // addi x1, x0, 10
        0xffc00113, // addi x2, x0, -4
        0x002081b3, // add x3, x1, x2 = 6
        0x40118233, // sub x4, x3, x1 = -4
        0x00309333, // sll x6, x1, x3 = 640
        0x001123b3, // slt x7, x2, x1 = 1
        0x0030c4b3, // xor x9, x1, x3 = 12
        0x0030e633, // or x12, x1, x3 = 14
        0x0030f6b3, // and x13, x1, x3 = 2
    ];
    let mut hw = load_program(&program);

    run_cycles(&mut hw, 120);

    assert_eq!(hw.soc.cpu.core0.regs.read(1), 10);
    assert_eq!(hw.soc.cpu.core0.regs.read(2), 0xffff_fffc);
    assert_eq!(hw.soc.cpu.core0.regs.read(3), 6);
    assert_eq!(hw.soc.cpu.core0.regs.read(4), 0xffff_fffc);
    assert_eq!(hw.soc.cpu.core0.regs.read(6), 640);
    assert_eq!(hw.soc.cpu.core0.regs.read(7), 1);
    assert_eq!(hw.soc.cpu.core0.regs.read(9), 12);
    assert_eq!(hw.soc.cpu.core0.regs.read(12), 14);
    assert_eq!(hw.soc.cpu.core0.regs.read(13), 2);
}

#[test]
fn branch_and_jump_update_pc_and_flush_wrong_path() {
    let program = [
        0x00500093, // addi x1, x0, 5
        0x00500113, // addi x2, x0, 5
        0x00208463, // beq x1, x2, 8
        0x06300193, // flushed: addi x3, x0, 99
        0x00a00213, // target: addi x4, x0, 10
        0x008000ef, // jal x1, 8
        0x06300293, // flushed by jal
        0x01400313, // jump target: addi x6, x0, 20
    ];
    let mut hw = load_program(&program);

    for _ in 0..8 {
        run_cycles(&mut hw, 120);
    }

    assert_eq!(hw.soc.cpu.core0.regs.read(3), 0);
    assert_eq!(hw.soc.cpu.core0.regs.read(4), 10);
    assert_eq!(hw.soc.cpu.core0.regs.read(5), 0);
    assert_eq!(hw.soc.cpu.core0.regs.read(6), 20);
}
