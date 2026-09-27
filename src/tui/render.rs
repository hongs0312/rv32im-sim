use super::panels::{pipeline, status, systolic};
use super::snapshot::Snapshot;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

pub fn draw(f: &mut Frame, snapshot: &Snapshot) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(f.size());

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[1]);

    pipeline::draw(f, main_chunks[0], snapshot);
    status::draw(f, right_chunks[0], snapshot);
    systolic::draw(f, right_chunks[1], snapshot);
}
