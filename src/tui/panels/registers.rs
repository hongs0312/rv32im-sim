use crate::tui::snapshot::Snapshot;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

// RISC-V 32개의 범용 레지스터 ABI 별명 배열
#[rustfmt::skip]
const ABI_NAMES: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2",
    "s0", "s1", "a0", "a1", "a2", "a3", "a4", "a5",
    "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7",
    "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6",
];

pub fn draw(f: &mut Frame, area: Rect, snapshot: &Snapshot) {
    // 4열 배치를 여유롭게 담기 위해 GPR을 75%로 약간 확장
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let mut gpr_text = String::new();
    for i in 0..32 {
        // "별명(x번호)" 형태의 문자열 생성 (예: "ra(x1)", "s11(x27)")
        let x_num = format!("(x{})", i);
        let reg_name = format!("{:<4} {:<5}", ABI_NAMES[i], x_num);

        // 너비를 9칸({:<9})으로 고정하여 줄을 반듯하게 맞춤
        gpr_text.push_str(&format!(
            "{} : 0x{:08X}   ",
            reg_name,
            snapshot.registers.read(i as u8)
        ));
        if (i + 1) % 3 == 0 {
            gpr_text.push('\n');
        }
    }

    let csr_text = format!(
        "SEPC    : 0x{:08X}\n\
         SCAUSE  : 0x{:08X}\n\
         STVAL   : 0x{:08X}\n\
         SSCRATCH: 0x{:08X}",
        snapshot.csr.sepc, snapshot.csr.scause, snapshot.csr.stval, snapshot.csr.sscratch
    );

    let gpr_block = Paragraph::new(gpr_text)
        .block(
            Block::default()
                .title(" General Purpose Registers ")
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::Yellow));

    let csr_block = Paragraph::new(csr_text)
        .block(
            Block::default()
                .title(" CSR (Supervisor) ")
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::Cyan));

    f.render_widget(gpr_block, chunks[0]);
    f.render_widget(csr_block, chunks[1]);
}
