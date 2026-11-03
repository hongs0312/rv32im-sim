use crate::tui::snapshot::Snapshot;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

pub fn draw(f: &mut Frame, area: Rect, snapshot: &Snapshot) {
    // 1. 위아래 테두리(Borders) 2줄을 제외한 실제 텍스트 출력 가능 높이 계산
    let inner_height = area.height.saturating_sub(2) as usize;

    // 2. 전체 로그 갯수에서 출력 가능 높이만큼만 '뒤에서부터' 슬라이싱 (오래된 윗줄 절삭)
    let start_idx = snapshot.logs.len().saturating_sub(inner_height);
    let visible_logs = &snapshot.logs[start_idx..];

    // 3. 잘라낸 최신 로그들만 문자열로 합치기
    let log_text = visible_logs.join("\n");

    let paragraph = Paragraph::new(log_text)
        .block(
            Block::default()
                .title("System Console / Logs")
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::LightCyan));
    // 참고: wrap(Wrap { trim: false })을 사용하면 긴 텍스트가 줄바꿈되면서
    // 계산해둔 inner_height와 실제 줄 수가 어긋날 수 있으므로 제거하는 것을 권장합니다.

    f.render_widget(paragraph, area);
}
