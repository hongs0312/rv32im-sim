use crate::hardware::soc::SoC;
use crate::hardware::soc::systolic::dma::DmaState;
use crate::hardware::soc::systolic::SystolicState;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub cycle: u64,
    pub pc: u32,
    pub pipeline: PipelineSnapshot,

    pub registers: [u32; 32],

    pub i_cache_state: String,
    pub d_cache_state: String,

    pub bus_state: String,
    pub bus_owner: String,

    pub systolic_state: String,
    pub systolic_progress: u16,
    pub systolic_grid: Vec<String>,
    pub sram_a_grid: Vec<String>,
    pub sram_b_grid: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PipelineSnapshot {
    pub if_id: String,
    pub id_ex: String,
    pub ex_mem: String,
    pub mem_wb: String,
    pub wb: String,
}

// SoC의 상태를 Snapshot으로 추출하는 메서드
impl From<&SoC> for Snapshot {
    fn from(soc: &SoC) -> Self {
        let if_id_str = if self.cpu.if_id_reg.instruction == 0x00000013 {
            "BUBBLE (NOP)".to_string()
        } else {
            format!("PC: 0x{:08X}", self.cpu.if_id_reg.pc)
        };

        let id_ex_str = if self.cpu.id_ex_reg.control.opcode == 0x13
            && self.cpu.id_ex_reg.control.funct3 == 0
        {
            "BUBBLE (NOP)".to_string()
        } else {
            format!(
                "PC: 0x{:08X} | rd: x{}",
                self.cpu.id_ex_reg.pc, self.cpu.id_ex_reg.rd
            )
        };

        let ex_mem_str = format!(
            "ALU Res: 0x{:08X} | rd: x{}",
            self.cpu.ex_mem_reg.alu_result, self.cpu.ex_mem_reg.rd
        );
        let mem_wb_str = format!(
            "WB Data: 0x{:08X} | rd: x{}",
            self.cpu.mem_wb_reg.mem_data, self.cpu.mem_wb_reg.rd
        );
        let wb_str = if self.cpu.mem_wb_reg.control.reg_write && self.cpu.mem_wb_reg.rd != 0 {
            let write_val = if self.cpu.mem_wb_reg.control.wb_src {
                self.cpu.mem_wb_reg.mem_data
            } else {
                self.cpu.mem_wb_reg.alu_result
            };
            format!("Write x{} = 0x{:08X}", self.cpu.mem_wb_reg.rd, write_val)
        } else {
            "BUBBLE / No Write".to_string()
        };

        let pipeline = PipelineSnapshot {
            if_id: if_id_str,
            id_ex: id_ex_str,
            ex_mem: ex_mem_str,
            mem_wb: mem_wb_str,
            wb: wb_str,
        };

        // 32개의 레지스터 값을 복사
        let mut registers = [0; 32];
        for i in 0..32 {
            registers[i] = self.cpu.regs.read(i as u8);
        }

        let i_cache_state_str = match &self.cpu.i_cache.state {
            cpu::elements::cache::CacheState::Idle => "Idle".to_string(),
            cpu::elements::cache::CacheState::WriteBack => "WriteBack".to_string(),
            cpu::elements::cache::CacheState::Fetch => "Fetch".to_string(),
        };

        let d_cache_state_str = match &self.cpu.d_cache.state {
            // I-Cache와 D-Cache 중 디버깅이 더 필요한 쪽을 선택하거나 둘 다 합칠 수 있습니다.
            cpu::elements::cache::CacheState::Idle => "Idle".to_string(),
            cpu::elements::cache::CacheState::WriteBack => "WriteBack".to_string(),
            cpu::elements::cache::CacheState::Fetch => "Fetch".to_string(),
        };

        let bus_state_str = match &self.bus_state {
            system_bus::BusState::Ready => "Ready".to_string(),
            system_bus::BusState::Processing(cycles) => format!("Busy ({} cycles)", cycles),
        };

        let bus_owner_str = match self.bus_owner {
            system_bus::BusOwner::None => "None",
            system_bus::BusOwner::ICache => "I-Cache",
            system_bus::BusOwner::DCache => "D-Cache",
            system_bus::BusOwner::SystolicDma => "Systolic DMA",
        }
        .to_string();

        let systolic_state_str = match &self.systolic.state {
            systolic::SystolicState::Idle => "Idle".to_string(),
            systolic::SystolicState::Loading => "DMA Loading".to_string(),
            systolic::SystolicState::Computing => "Computing".to_string(),
            systolic::SystolicState::Storing => "DMA Storing".to_string(),
            systolic::SystolicState::Done => "Done".to_string(),
        };

        use crate::hardware::soc::systolic::dma::DmaState;
        use crate::hardware::soc::systolic::SystolicState;

        let progress_f32 = match self.systolic.state {
            SystolicState::Idle => 0.0,
            
            SystolicState::Loading => {
                // A와 B 행렬 로드 (총 256 + 256 = 512 워드) -> 0% ~ 40% 구간
                let words_loaded = match self.systolic.dma.state {
                    DmaState::LatencyWait { is_a: true, .. } => 0,
                    DmaState::Bursting { is_a: true, row, col } => row * 16 + col,
                    DmaState::LatencyWait { is_a: false, .. } => 256,
                    DmaState::Bursting { is_a: false, row, col } => 256 + row * 16 + col,
                    DmaState::Done => 512,
                    _ => 0,
                };
                (words_loaded as f32 / 512.0) * 40.0
            }
            
            SystolicState::Computing => {
                // 시스톨릭 연산 사이클 (최대 46 사이클) -> 40% ~ 80% 구간
                let max_cycle = (16 * 2 + 16 - 2) as f32; // ARRAY_SIZE * 2 + INNER_DIM - 2
                let cur_cycle = self.systolic.cycle as f32;
                40.0 + (cur_cycle / max_cycle) * 40.0
            }
            
            SystolicState::Storing => {
                // 결과 C 행렬 저장 (총 256 워드) -> 80% ~ 100% 구간
                let words_stored = match self.systolic.dma.state {
                    DmaState::StoringC { row, col } => row * 16 + col,
                    DmaState::Done => 256,
                    _ => 0,
                };
                80.0 + (words_stored as f32 / 256.0) * 20.0
            }
            
            SystolicState::Done => 100.0,
        };

        // Snapshot 필드에 넣을 때 u16으로 변환하여 clamp 처리
        let systolic_progress = progress_f32.clamp(0.0, 100.0) as u16;

        // --- 16x16 PE Array, SRAM A, SRAM B 그리드 생성 ---
        let mut systolic_grid = Vec::new();
        let mut sram_a_grid = Vec::new();
        let mut sram_b_grid = Vec::new();

        for i in 0..16 {
            let mut pe_row = String::new();
            let mut a_row = String::new();
            let mut b_row = String::new();

            for j in 0..16 {
                // [PE Array] 값(psum)을 보는 것이 아니라 순수 하드웨어 제어 신호(is_active)만 확인
                if self.systolic.pes[i][j].is_active {
                    pe_row.push_str("■ ");
                } else {
                    pe_row.push_str("□ ");
                }

                // [SRAM A] DMA가 채워넣은 valid 비트가 true면 켜짐, PE로 방출되어 false가 되면 꺼짐
                if self.systolic.scratchpad.a_valid[i][j] {
                    a_row.push_str("■ ");
                } else {
                    a_row.push_str("□ ");
                }

                // [SRAM B] 마찬가지로 방출 여부만 순수하게 확인
                if self.systolic.scratchpad.b_valid[i][j] {
                    b_row.push_str("■ ");
                } else {
                    b_row.push_str("□ ");
                }
            }
            systolic_grid.push(pe_row);
            sram_a_grid.push(a_row);
            sram_b_grid.push(b_row);
        }

        Snapshot {
            cycle: self.cycle,
            pc: self.cpu.pc,
            pipeline,

            registers,

            i_cache_state: i_cache_state_str,
            d_cache_state: d_cache_state_str,

            bus_state: bus_state_str,
            bus_owner: bus_owner_str,

            systolic_state: systolic_state_str,
            systolic_progress: systolic_progress,
            systolic_grid: systolic_grid,
            sram_a_grid: sram_a_grid,
            sram_b_grid: sram_b_grid,
        }
    }
}

