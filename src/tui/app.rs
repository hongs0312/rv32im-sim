use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, time::Duration};

use super::render;
use crate::{hardware::soc::SoC, tui::snapshot::Snapshot};

pub fn run_tui(mut soc: SoC) -> io::Result<()> {
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

    loop {
        // 1. 현재 SoC 상태를 Snapshot으로 추출
        let snapshot = Snapshot::from(&soc);

        // 2. 화면 그리기
        terminal.draw(|f| render::draw(f, &snapshot))?;

        // ecall이 파이프라인 끝단에 도달했는지 확인
        if soc.cpu.mem_wb_reg.control.is_ecall || soc.cpu.ex_mem_reg.control.is_ecall {
            // 시뮬레이션을 완전히 끝내고 즉시 종료하려면 break를 활성화합니다.
            // break;

            // 만약 종료하지 않고 마지막 레지스터 상태를 TUI 화면에서 감상하고 싶다면
            // break 대신 아래처럼 자동 재생만 끄고 입력 대기 상태로 둘 수 있습니다.
            auto_run = false;
        }

        match auto_run {
            true => {
                soc.tick();

                // 자동 재생 중에도 입력이 있는지 아주 짧게 검사 (1ms 대기)
                if event::poll(Duration::from_millis(1))? {
                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Char('a') | KeyCode::Char(' ') => auto_run = false, // 'a' 또는 스페이스바 누르면 자동 실행 중지
                            KeyCode::Char('q') => break, // 'q' 누르면 종료
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
            }
            false => {
                if event::poll(Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        match key.code {
                            KeyCode::Char('q') => break, // 'q' 누르면 종료
                            KeyCode::Char('c')
                                if key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
                            {
                                break;
                            }
                            KeyCode::Char(' ') | KeyCode::Right => soc.tick(), // 스페이스바 또는 오른쪽 화살표 누르면 1 Cycle 실행
                            KeyCode::Enter => {
                                // 빨리 감기: 한 번에 1000 Cycle 실행
                                for _ in 0..1000 {
                                    soc.tick();
                                }
                            }
                            KeyCode::Char('a') => auto_run = true, // 'a' 누르면 자동 실행 시작
                            _ => {}
                        }
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
