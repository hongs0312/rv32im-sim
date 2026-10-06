use crate::hardware::soc::types::OpStatus;

pub const ROW_SIZE: usize = 1024; // Size of a row in bytes

const RP_CYCLES: u8 = 3; // Precharge time in cycles
const RCD_CYCLES: u8 = 3; // Row to Column Delay in cycles
const CAS_CYCLES: u8 = 3; // Column Access Strobe time

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankState {
    Idle,
    Precharging {
        cycles_left: u8,
        target_row: usize,
        target_offset: usize,
        is_write: bool,
    }, // tRP
    Activating {
        cycles_left: u8,
        target_row: usize,
        target_offset: usize,
        is_write: bool,
    }, // tRCD
    Accessing {
        cycles_left: u8,
        target_row: usize,
        target_offset: usize,
        is_write: bool,
    }, // tCAS
}

pub struct MemoryBank {
    pub data: Vec<u8>,
    pub state: BankState,
    pub active_row: Option<usize>,
    pub write_buffer: [u8; 16], // 쓰기 요청 시 뱅크가 데이터를 임시 보관하는 래치(Latch)
}

impl MemoryBank {
    pub fn new(num_rows: usize) -> Self {
        Self {
            data: vec![0; num_rows * ROW_SIZE],
            state: BankState::Idle,
            active_row: None,
            write_buffer: [0; 16],
        }
    }

    #[rustfmt::skip]
    pub fn tick(&mut self) {
        match self.state {
            BankState::Idle => {}
            BankState::Precharging { cycles_left, target_row, target_offset, is_write } => {
                if cycles_left > 1 {
                    self.state = BankState::Precharging { cycles_left: cycles_left - 1, target_row, target_offset, is_write };
                } else {
                    self.active_row = None;
                    self.state = BankState::Activating { cycles_left: RCD_CYCLES, target_row, target_offset, is_write };
                }
            }

            BankState::Activating { cycles_left, target_row, target_offset, is_write } => {
                if cycles_left > 1 {
                    self.state = BankState::Activating { cycles_left: cycles_left - 1, target_row, target_offset, is_write };
                } else {
                    self.active_row = Some(target_row);
                    self.state = BankState::Accessing { cycles_left: CAS_CYCLES, target_row, target_offset, is_write };
                }
            }

            BankState::Accessing { cycles_left, target_row, target_offset, is_write } => {
                if cycles_left > 0 {
                    self.state = BankState::Accessing { cycles_left: cycles_left - 1, target_row, target_offset, is_write };
                }
            }
        }
    }

    pub fn issue_read(&mut self, row: usize, offset: usize) -> OpStatus<()> {
        if self.state != BankState::Idle {
            return OpStatus::Busy;
        }
        self.start_transaction(row, offset, false);
        OpStatus::Complete(())
    }

    pub fn issue_write(&mut self, row: usize, offset: usize, block: &[u8; 16]) -> OpStatus<()> {
        if self.state != BankState::Idle {
            return OpStatus::Busy;
        }
        self.write_buffer.copy_from_slice(block);
        self.start_transaction(row, offset, true);
        OpStatus::Complete(())
    }

    #[rustfmt::skip]
    fn start_transaction(&mut self, row: usize, offset: usize, is_write: bool) {
        if self.active_row == Some(row) {
            self.state = BankState::Accessing { cycles_left: CAS_CYCLES, target_row: row, target_offset: offset, is_write };
        } else if self.active_row.is_some() {
            self.state = BankState::Precharging { cycles_left: RP_CYCLES, target_row: row, target_offset: offset, is_write };
        } else {
            self.state = BankState::Activating { cycles_left: RCD_CYCLES, target_row: row, target_offset: offset, is_write };
        }
    }

    #[rustfmt::skip]
    pub fn collect_read(&mut self, row: usize, offset: usize) -> OpStatus<[u8; 16]> {
        // println!("[BANK COLLECT TRY] Target Row: {}, Offset: {}, Bank State: {:?}", row, offset, self.state);

        match self.state {
            // 지연 시간이 0이 되었고, 주소와 작업 종류가 일치할 때만 데이터 내어줌
            BankState::Accessing { cycles_left: 0, target_row, target_offset, is_write: false }
                if target_row == row && target_offset == offset =>
            {
                self.state = BankState::Idle;
                let mut block = [0u8; 16];
                let base = row * ROW_SIZE + offset;
                block.copy_from_slice(&self.data[base..base + 16]);
                OpStatus::Complete(block)
            }
            _ => OpStatus::Busy, // 아직 계산 중이거나 남의 주소면 쫓아냄
        }
    }

    #[rustfmt::skip]
    pub fn collect_write(&mut self, row: usize, offset: usize) -> OpStatus<()> {
        match self.state {
            BankState::Accessing { cycles_left: 0, target_row, target_offset, is_write: true }
                if target_row == row && target_offset == offset =>
            {
                self.state = BankState::Idle;
                let base = row * ROW_SIZE + offset;

                // 💡 임시 보관해두었던 write_buffer의 데이터를 물리 메모리에 최종 기록
                self.data[base..base + 16].copy_from_slice(&self.write_buffer);
                OpStatus::Complete(())
            }
            _ => OpStatus::Busy,
        }
    }
}
