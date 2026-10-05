const ROW_SIZE: usize = 1024; // Size of a row in bytes

#[derive(Clone, Copy, Debug)]
pub struct Row {
    data: [u8; ROW_SIZE],
}

impl Row {
    pub fn read32(&self, col_offset: usize) -> u32 {
        let start = col_offset;
        let end = col_offset + 4;
        let bytes = &self.data[start..end];
        
        u32::from_le_bytes(bytes.try_into().expect("Slice with incorrect length"))
    }

    pub fn write32_masked(&mut self, col_offset: usize, value: u32, mask: u8) {
        let bytes = value.to_le_bytes();

        for i in 0..4 {
            if (mask & (1 << i)) != 0 {
                self.data[col_offset + i] = bytes[i];
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankState {
    Idle,
    Activating { cycles: u8, target_row: usize },
    Reading { cycles: u8, target_col: usize },
    Writing { cycles: u8, target_col: usize, data: u32 },
    Precharging { cycles: u8 },
}

pub struct MemoryBank {
    data: Vec<Row>,

    pub open_row_id: Option<usize>,

    pub output_latch: Option<u32>,

    pub state: BankState,
}

impl MemoryBank {
    pub fn new(num_rows: usize) -> Self {
        Self {
            data: vec![Row { data: [0; ROW_SIZE] }; num_rows],
            open_row_id: None,
            output_latch: None,
            state: BankState::Idle,
        }
    }
    pub fn step(&mut self) {
        match self.state {
            BankState::Activating { cycles, target_row} => {
                if cycles > 0 {
                    self.state = BankState::Activating { cycles: cycles - 1, target_row: target_row };
                } else {
                    self.open_row_id = Some(target_row);
                    self.state = BankState::Idle;
                }
            }
            BankState::Reading { cycles, target_col } => {
                if cycles > 0 {
                    self.state = BankState::Reading { cycles: cycles - 1, target_col: target_col };
                } else {
                    if let Some(row_id) = self.open_row_id {
                        let read_data = &self.data[row_id].read_byte(target_col);
                        self.output_latch = Some(u32::from_le_bytes(read_data.try_into().expect("Slice with incorrect length")));
                    }
                    self.state = BankState::Idle;
                }
            }
            BankState::Writing { cycles, target_col, data } => {
                if cycles > 0 {
                    self.state = BankState::Writing { cycles: cycles - 1, target_col: target_col, data: data };
                } else {
                    if let Some(row_id) = self.open_row_id {
                        self.data[row_id].write32_masked(target_col, data, 0xFF);
                    }
                    self.state = BankState::Idle;
                }
            }
            BankState::Precharging { cycles } => {
                if cycles > 0 {
                    self.state = BankState::Precharging { cycles: cycles - 1 };
                } else {
                    self.state = BankState::Idle;
                }
            }
            BankState::Idle => {}
        }
    }
}

