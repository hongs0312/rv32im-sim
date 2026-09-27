use crate::tui::snapshot::Snapshot;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

pub fn draw(f: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let block = Block::default()
        .title(" System Stats & Bus ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let stats = vec![
        Line::from(format!("Global Cycle : {}", snapshot.cycle)),
        Line::from(format!("PC           : 0x{:08X}", snapshot.pc)),
        Line::from(format!("Bus State    : {}", snapshot.bus_state)),
        Line::from(format!("Bus Owner    : {}", snapshot.bus_owner)),
        Line::from(format!("I-Cache State: {}", snapshot.i_cache_state)),
        Line::from(format!("D-Cache State: {}", snapshot.d_cache_state)),
    ];

    let paragraph = Paragraph::new(stats).block(block);
    f.render_widget(paragraph, area);
}
