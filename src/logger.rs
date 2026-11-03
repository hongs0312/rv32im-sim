use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

// 💡 TUI 모드가 켜졌는지 확인하는 전역 플래그 (기본값: false)
pub static IS_TUI_MODE: AtomicBool = AtomicBool::new(false);

pub static SIM_LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
pub static UART_BUF: OnceLock<Mutex<String>> = OnceLock::new();

pub fn get_logs() -> &'static Mutex<Vec<String>> {
    SIM_LOGS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn get_uart_buf() -> &'static Mutex<String> {
    UART_BUF.get_or_init(|| Mutex::new(String::new()))
}

#[macro_export]
macro_rules! sim_log {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        // 💡 TUI 모드면 패널 버퍼에 담고, 아니면 기존처럼 println! 실행
        if $crate::logger::IS_TUI_MODE.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(mut logs) = $crate::logger::get_logs().lock() {
                if logs.len() > 500 {
                    logs.remove(0);
                }
                logs.push(msg);
            }
        } else {
            println!("{}", msg);
        }
    };
}

pub fn log_char(c: u8) {
    if IS_TUI_MODE.load(Ordering::Relaxed) {
        // TUI 모드: 버퍼에 모았다가 TUI 콘솔로 출력
        if let Ok(mut buf) = get_uart_buf().lock() {
            if c == b'\n' {
                sim_log!("{}", buf);
                buf.clear();
            } else {
                buf.push(c as char);
            }
        }
    } else {
        // 일반 모드: 즉시 표준 출력(stdout)
        print!("{}", c as char);
        let _ = io::stdout().flush();
    }
}
