use super::panels::{consol, pipeline, registers, status, systolic}; // registers 모듈 추가
use super::snapshot::Snapshot;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

// UI 전환 상태를 나타내는 열거형
pub enum RightBottomView {
    Systolic,
    Registers,
}

// 렌더링 함수에 뷰 모드 파라미터 추가
pub fn draw(f: &mut Frame, snapshot: &Snapshot, view_mode: &RightBottomView) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(f.size());

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(main_chunks[0]);

    let core_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // Pipeline Stages (IF, ID, EX, MEM, WB)
            Constraint::Percentage(40), // System Stats & Bus (PC, Cycle, Cache 상태)
        ])
        .split(left_chunks[0]);

    pipeline::draw(f, core_chunks[0], snapshot);

    status::draw(f, core_chunks[1], snapshot);

    // 좌측 하단: 새로 만든 콘솔 창 렌더링
    consol::draw(f, left_chunks[1], snapshot);

    // 뷰 모드에 따른 조건부 렌더링 토글
    match view_mode {
        RightBottomView::Systolic => systolic::draw(f, main_chunks[1], snapshot),
        RightBottomView::Registers => registers::draw(f, main_chunks[1], snapshot),
    }
}
