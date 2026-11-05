#[derive(Clone, Copy, Debug)]
pub struct StreamBuffer {
    pub valid: bool,
    pub fetching: bool,
    pub block_addr: u32,
    pub data: [u8; 16],
}

impl StreamBuffer {
    pub fn new() -> Self {
        Self {
            valid: false,
            fetching: false,
            block_addr: 0,
            data: [0; 16],
        }
    }
}
