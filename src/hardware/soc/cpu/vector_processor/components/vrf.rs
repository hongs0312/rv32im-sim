pub const VLEN_BITS: usize = 128;
pub const VLEN_BYTES: usize = VLEN_BITS / 8;
pub const NUM_VREGS: usize = 32;

pub struct Vrf {
    registers: [[u8; VLEN_BYTES]; NUM_VREGS],

    // 파이프라인 (V-WB 단계)에서 값을 기록하기 위한 통신포트
    write_port: Option<(usize, [u8; VLEN_BYTES])>,
}

impl Vrf {
    pub fn new() -> Self {
        Self {
            registers: [[0; VLEN_BYTES]; NUM_VREGS],
            write_port: None,
        }
    }

    #[inline]
    pub fn read(&self, index: usize) -> [u8; VLEN_BYTES] {
        if index >= NUM_VREGS {
            panic!("유효하지 않은 벡터 레지스터 주소: {}", index);
        }

        if let Some((w_idx, w_data)) = self.write_port {
            if w_idx == index {
                return w_data;
            }
        }

        self.registers[index]
    }

    #[inline]
    pub fn write_port(&mut self, index: usize, data: [u8; VLEN_BYTES]) {
        if index < NUM_VREGS {
            self.write_port = Some((index, data));
        }
    }

    pub fn tick(&mut self) {
        if let Some((index, data)) = self.write_port.take() {
            self.registers[index] = data;
        }
    }
}
