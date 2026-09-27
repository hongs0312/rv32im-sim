/*
    register.rs
    - 이 파일은 CPU의 레지스터 파일(Register File)과 파이프라인 레지스터 구조체들을 정의합니다.
    - RegisterFile 구조체는 32개의 32비트 레지스터를 관리하며, x0 레지스터는 항상 0으로 유지됩니다.
    - 파이프라인 레지스터 구조체들은 각 파이프라인 단계에서 필요한 데이터를 저장하고 전달하는 역할을 합니다.
*/

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct RegisterFile {
    regs: [u32; 32],
}

impl RegisterFile {
    pub fn new() -> Self {
        Self { regs: [0; 32] }
    }

    pub fn read(&self, reg: u8) -> u32 {
        if reg == 0 { 0 } else { self.regs[reg as usize] }
    }

    pub fn write(&mut self, reg: u8, value: u32, write_enable: bool) {
        // x0 레지스터는 항상 0이어야 하므로, x0에 쓰기를 시도하면 무시
        if reg != 0 && write_enable {
            self.regs[reg as usize] = value;
        }
    }
}
