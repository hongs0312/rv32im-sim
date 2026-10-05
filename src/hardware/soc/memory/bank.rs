use crate::hardware::soc::types::OpStatus;

pub const ROW_SIZE: usize = 1024; // Size of a row in bytes

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BankState {
    Idle,
    Precharging { cycles_left: u8, next_row: usize }, // 기존 Row 닫기 (tRP)
    Activating { cycles_left: u8, target_row: usize }, // 새 Row 열기 (tRCD)
    Accessing { cycles_left: u8 },                    // 읽기/쓰기 (tCAS)
}

pub struct MemoryBank {
    pub data: Vec<u8>,
    pub state: BankState,
    pub active_row: Option<usize>,
}

impl MemoryBank {
    pub fn new(num_rows: usize) -> Self {
        Self {
            data: vec![0; num_rows * ROW_SIZE],
            state: BankState::Idle,
            active_row: None,
        }
    }

    pub fn read_block(&mut self, row: usize, offset: usize) -> OpStatus<[u8; 16]> {
        match self.state {
            BankState::Accessing { cycles_left: 1 } => {
                self.state = BankState::Idle;

                let mut block = [0u8; 16];
                let base = row * ROW_SIZE + offset;
                block.copy_from_slice(&self.data[base..base + 16]);

                OpStatus::Complete(block)
            }
            _ => {
                self.handle_latency(row);

                OpStatus::Busy
            }
        }
    }

    pub fn write_block(&mut self, row: usize, offset: usize, block: &[u8; 16]) -> OpStatus<()> {
        match self.state {
            BankState::Accessing { cycles_left: 1 } => {
                self.state = BankState::Idle;

                // 💡 읽기와 반대로 입력받은 block을 뱅크의 data에 덮어씀
                let base = row * ROW_SIZE + offset;
                self.data[base..base + 16].copy_from_slice(block);

                OpStatus::Complete(()) // 쓰기 완료 반환
            }
            _ => {
                self.handle_latency(row);

                OpStatus::Busy
            }
        }
    }

    #[rustfmt::skip]
    fn handle_latency(&mut self, row: usize) {
        match self.state {
            BankState::Idle => {
                if self.active_row == Some(row) {
                    self.state = BankState::Accessing { cycles_left: 2 } // tCAS
                } else if self.active_row.is_some() {
                    self.state = BankState::Precharging { cycles_left: 2, next_row: row } // tRP
                } else {
                    self.state = BankState::Activating { cycles_left: 2, target_row: row } // tRCD
                }
            }
            BankState::Precharging { cycles_left, next_row } => {
                if cycles_left > 1 {
                    self.state = BankState::Precharging { cycles_left: cycles_left - 1, next_row };
                } else {
                    self.active_row = None;
                    self.state = BankState::Activating { cycles_left: 2, target_row: next_row }; // tRCD
                }
            },
            BankState::Activating { cycles_left, target_row } => {
                if cycles_left > 1 {
                    self.state = BankState::Activating { cycles_left: cycles_left - 1, target_row };
                } else {
                    self.active_row = Some(target_row);
                    self.state = BankState::Accessing { cycles_left: 2 }; // tCAS
                }
            },
            BankState::Accessing { cycles_left } => {
                self.state = BankState::Accessing { cycles_left: cycles_left - 1 };
            }
        }
    }
}
