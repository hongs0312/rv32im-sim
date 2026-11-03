use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, time::Duration};

use super::render;

use crate::{hardware::Hardware, sim_log, tui::snapshot::Snapshot};

pub fn run_tui(mut hardware: Hardware) -> io::Result<()> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // 터미널 초기화 설정
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut auto_run = false; // 자동 실행 모드 여부
    let mut view_mode = render::RightBottomView::Systolic; // 초기 뷰 모드 설정

    loop {
        // 1. 현재 SoC 상태를 Snapshot으로 추출 (기존 Snapshot::from(&SoC) 재사용)
        let snapshot = Snapshot::from(&hardware.soc);

        // 2. 화면 그리기
        terminal.draw(|f| render::draw(f, &snapshot, &mut view_mode))?;

        if hardware.syscon.is_halted {
            auto_run = false;
        }

        match auto_run {
            true => {
                // 💡 최상위 메인보드 틱 호출
                hardware.tick();

                // 자동 재생 중에도 입력이 있는지 아주 짧게 검사 (1ms 대기)
                if event::poll(Duration::from_millis(1))?
                    && let Event::Key(key) = event::read()?
                {
                    match key.code {
                        KeyCode::Char('a') | KeyCode::Char(' ') => auto_run = false,
                        KeyCode::Char('q') => break,
                        KeyCode::Char('c')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            break;
                        }
                        _ => {}
                    }
                }
            }
            false => {
                if event::poll(Duration::from_millis(100))?
                    && let Event::Key(key) = event::read()?
                {
                    match key.code {
                        KeyCode::Tab => {
                            view_mode = match view_mode {
                                render::RightBottomView::Systolic => {
                                    render::RightBottomView::Registers
                                }
                                render::RightBottomView::Registers => {
                                    render::RightBottomView::Systolic
                                }
                            };
                        }
                        KeyCode::Char('q') => break,
                        KeyCode::Char('c')
                            if key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            break;
                        }
                        // 💡 스페이스바 또는 오른쪽 화살표 누르면 1 Cycle 실행
                        KeyCode::Char(' ') | KeyCode::Right => hardware.tick(),
                        KeyCode::Enter => {
                            // 빨리 감기: 한 번에 100 Cycle 실행
                            for _ in 0..1000 {
                                if hardware.syscon.is_halted {
                                    sim_log!(
                                        "[TUI] 프로그램이 종료되었습니다. (PC: 0x{:08X})",
                                        hardware.soc.cpu.core0.pc
                                    );
                                    break;
                                }
                                hardware.tick();
                            }
                        }
                        KeyCode::Char('a') => auto_run = true,
                        _ => {}
                    }
                }
            }
        }
    }

    // 터미널 정리 후 원래 상태로 복구
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
