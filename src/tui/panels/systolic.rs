use crate::tui::snapshot::Snapshot;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, LineGauge, Paragraph}, // Gauge 대신 LineGauge 사용
};

pub fn draw(f: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let block = Block::default()
        .title(" Systolic Array Accelerator ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // 세로 공간 확보를 위해 프로그레스 바를 1줄(LineGauge)로 축소
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(18), // Length 대신 Min을 사용하여 강제 축소 방지
            Constraint::Min(18),
            Constraint::Length(3), // 얇은 게이지용 3줄
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

    // 두꺼운 Gauge 대신 1줄짜리 LineGauge로 교체하여 세로 2줄 절약
    let gauge = LineGauge::default()
        .block(Block::default().title(" Total Progress "))
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
        .line_set(ratatui::symbols::line::THICK)
        .ratio(snapshot.systolic_progress as f64 / 100.0); // LineGauge는 ratio(0.0~1.0) 사용
    f.render_widget(gauge, vertical_chunks[2]);
}
