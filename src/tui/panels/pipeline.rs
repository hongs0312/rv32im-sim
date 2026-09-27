use crate::tui::snapshot::Snapshot;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};

pub fn draw(f: &mut Frame, area: Rect, snapshot: &Snapshot) {
    let block = Block::default()
        .title(" Pipline Stages ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    //5단계 파이프라인을 수직으로 균등 분할
    let stages = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
        ])
        .split(inner_area);

    // 각 단계 렌더링 (추후 Snapshot의 데이터를 기반으로 실제 상태를 표시하도록 수정)
    let pipeline_data = [
        ("IF", &snapshot.pipeline.if_id, Color::White),
        ("ID", &snapshot.pipeline.id_ex, Color::White),
        ("EX", &snapshot.pipeline.ex_mem, Color::Yellow),
        ("MEM", &snapshot.pipeline.mem_wb, Color::Magenta),
        ("WB", &snapshot.pipeline.wb, Color::Green),
    ];

    for (i, (name, data, default_color)) in pipeline_data.iter().enumerate() {
        let stage_block = Block::default().title(*name).borders(Borders::ALL);

        let color = if data.contains("BUBBLE") {
            Color::DarkGray
        } else {
            *default_color
        };

        let content = Paragraph::new(Line::from(format!("  {}", data)))
            .block(stage_block)
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD));
        f.render_widget(content, stages[i]);
    }
}
