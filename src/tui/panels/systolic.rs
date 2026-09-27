use crate::tui::snapshot::Snapshot;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Gauge, Paragraph},
};

pub fn draw(f: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let block = Block::default()
        .title(" Systolic Array Accelerator ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(inner_area);

    let top_horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(vertical_chunks[0]);

    let status_text = format!(
        "State: {}\nDMA 로드 및 웨이브프론트 연산중...",
        snapshot.systolic_state
    );
    f.render_widget(
        Paragraph::new(status_text).style(Style::default().fg(Color::Green)),
        top_horizontal[0],
    );

    let b_lines: Vec<Line> = snapshot
        .sram_b_grid
        .iter()
        .map(|row| Line::from(row.as_str()))
        .collect();
    f.render_widget(
        Paragraph::new(b_lines)
            .block(
                Block::default()
                    .title("SRAM B (Weight)")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::Magenta)),
        top_horizontal[1],
    );

    let bottom_horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(vertical_chunks[1]);

    let a_lines: Vec<Line> = snapshot
        .sram_a_grid
        .iter()
        .map(|row| Line::from(row.as_str()))
        .collect();
    f.render_widget(
        Paragraph::new(a_lines)
            .block(
                Block::default()
                    .title("SRAM A (Input)")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::Cyan)),
        bottom_horizontal[0],
    );

    let pe_lines: Vec<Line> = snapshot
        .systolic_grid
        .iter()
        .map(|row| Line::from(row.as_str()))
        .collect();
    f.render_widget(
        Paragraph::new(pe_lines)
            .block(
                Block::default()
                    .title("PE Array (16x16)")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::Yellow)),
        bottom_horizontal[1],
    );

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title("Total Progress")
                .borders(Borders::ALL),
        )
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
        .percent(snapshot.systolic_progress);
    f.render_widget(gauge, vertical_chunks[2]);
}
