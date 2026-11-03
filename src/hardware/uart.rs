use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, poll, read as read_event}; // KeyModifiers 추가
use crossterm::terminal::disable_raw_mode;
use std::io::{self, Write};
use std::time::Duration;

use super::mmio_device::MmioDevice;

pub const UART_BASE: u32 = 0x1000_0000;
pub const UART_SIZE: u32 = 0x100;

pub struct Uart {
    rx_buffer: Option<u8>, // 수신 버퍼
}

impl Uart {
    pub fn new() -> Self {
        Self { rx_buffer: None }
    }

    // 터미널 입력을 논블로킹으로 확인하여 버퍼에 채우는 함수
    fn fill_rx_buffer(&mut self) {
        if self.rx_buffer.is_some() {
            return; // 이미 읽은 글자가 있으면 무시
        }

        if let Ok(true) = poll(Duration::from_secs(0)) {
            if let Ok(Event::Key(key_event)) = read_event() {
                // 💡 키를 누를 때(Press)만 처리하여 중복 입력 방지
                if key_event.kind == KeyEventKind::Press {
                    if key_event.code == KeyCode::Char('c')
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        let _ = disable_raw_mode(); // 터미널을 정상으로 되돌림
                        println!("\r\n[Emulator] Terminated by Ctrl+C");
                        std::process::exit(0); // 프로세스 완전 종료
                    }

                    match key_event.code {
                        KeyCode::Char(c) => self.rx_buffer = Some(c as u8),
                        // 💡 \n을 \r로 변경 (Bare-metal 쉘 표준)
                        KeyCode::Enter => self.rx_buffer = Some(b'\r'),
                        _ => {}
                    }
                }
            }
        }
    }
}

impl Default for Uart {
    fn default() -> Self {
        Self::new()
    }
}

impl MmioDevice for Uart {
    fn read(&mut self, offset: u32) -> Result<u32, String> {
        self.fill_rx_buffer(); // 읽기 요청이 올 때마다 키보드 입력 확인

        match offset {
            0 => {
                // RHR (Receiver Holding Register): 글자를 읽어감
                if let Some(c) = self.rx_buffer.take() {
                    // 읽은 뒤 버퍼 비움
                    Ok(c as u32)
                } else {
                    Ok(0)
                }
            }
            5 => {
                // LSR (Line Status Register): 상태 반환
                if self.rx_buffer.is_some() {
                    Ok(0x01) // Bit 0 (Data Ready) = 1
                } else {
                    Ok(0x00) // 아무 글자도 없음 = 0
                }
            }
            _ => Ok(0), // 기타 레지스터는 무시
        }
    }

    fn write(&mut self, _offset: u32, value: u32) -> Result<(), String> {
        let c = value as u8;

        // 줄바꿈(\n) 문자가 들어오면 커서를 맨 앞으로 당기는 캐리지 리턴(\r)을 먼저 출력
        if c == b'\n' {
            crate::logger::log_char(b'\r');
        }

        crate::logger::log_char(c);
        let _ = io::stdout().flush();
        Ok(())
    }
}
