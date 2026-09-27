use crate::hardware::soc::SoC;
use crate::hardware::soc::cpu::elements::cache::CacheState;
use crate::hardware::soc::system_bus::{BusOwner, BusState};
use crate::hardware::soc::systolic::SystolicState;
use crate::hardware::soc::systolic::dma::DmaState;

use crate::hardware::soc::systolic::{ARRAY_SIZE, INNER_DIM};

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

impl From<&SoC> for Snapshot {
    fn from(soc: &SoC) -> Self {
        // --- 1. Pipeline Stages ---
        let if_id_str = if soc.cpu.if_id_reg.instruction == 0x00000013 {
            "BUBBLE (NOP)".to_string()
        } else {
            format!("PC: 0x{:08X}", soc.cpu.if_id_reg.pc)
        };

        let id_ex_str =
            if soc.cpu.id_ex_reg.control.opcode == 0x13 && soc.cpu.id_ex_reg.control.funct3 == 0 {
                "BUBBLE (NOP)".to_string()
            } else {
                format!(
                    "PC: 0x{:08X} | rd: x{}",
                    soc.cpu.id_ex_reg.pc, soc.cpu.id_ex_reg.rd
                )
            };

        let ex_mem_str = format!(
            "ALU Res: 0x{:08X} | rd: x{}",
            soc.cpu.ex_mem_reg.alu_result, soc.cpu.ex_mem_reg.rd
        );
        let mem_wb_str = format!(
            "WB Data: 0x{:08X} | rd: x{}",
            soc.cpu.mem_wb_reg.mem_data, soc.cpu.mem_wb_reg.rd
        );
        let wb_str = if soc.cpu.mem_wb_reg.control.reg_write && soc.cpu.mem_wb_reg.rd != 0 {
            let write_val = if soc.cpu.mem_wb_reg.control.wb_src {
                soc.cpu.mem_wb_reg.mem_data
            } else {
                soc.cpu.mem_wb_reg.alu_result
            };
            format!("Write x{} = 0x{:08X}", soc.cpu.mem_wb_reg.rd, write_val)
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

        // --- 2. Register File ---
        let mut registers = [0; 32];
        for i in 0..32 {
            registers[i] = soc.cpu.regs.read(i as u8);
        }

        // --- 3. Caches ---
        let i_cache_state_str = match &soc.cpu.i_cache.state {
            CacheState::Idle => "Idle".to_string(),
            CacheState::WriteBack => "WriteBack".to_string(),
            CacheState::Fetch => "Fetch".to_string(),
        };

        let d_cache_state_str = match &soc.cpu.d_cache.state {
            CacheState::Idle => "Idle".to_string(),
            CacheState::WriteBack => "WriteBack".to_string(),
            CacheState::Fetch => "Fetch".to_string(),
        };

        // --- 4. Bus State & Owner ---
        let bus_state_str = match &soc.bus_state {
            BusState::Ready => "Ready".to_string(),
            BusState::Processing(cycles) => format!("Busy ({} cycles)", cycles),
        };

        let bus_owner_str = match soc.bus_owner {
            BusOwner::None => "None",
            BusOwner::ICache => "I-Cache",
            BusOwner::DCache => "D-Cache",
            BusOwner::SystolicDma => "Systolic DMA",
        }
        .to_string();

        // --- 5. Systolic State & Progress ---
        let systolic_state_str = match &soc.systolic.state {
            SystolicState::Idle => "Idle".to_string(),
            SystolicState::Loading => "DMA Loading".to_string(),
            SystolicState::Computing => "Computing".to_string(),
            SystolicState::Storing => "DMA Storing".to_string(),
            SystolicState::Done => "Done".to_string(),
        };

        let progress_f32 = match soc.systolic.state {
            SystolicState::Idle => 0.0,
            SystolicState::Loading => {
                let words_loaded = match soc.systolic.dma.state {
                    DmaState::LatencyWait { is_a: true, .. } => 0,
                    DmaState::Bursting {
                        is_a: true,
                        row,
                        col,
                    } => row * 16 + col,
                    DmaState::LatencyWait { is_a: false, .. } => 256,
                    DmaState::Bursting {
                        is_a: false,
                        row,
                        col,
                    } => 256 + row * 16 + col,
                    DmaState::Done => 512,
                    _ => 0,
                };
                (words_loaded as f32 / 512.0) * 40.0
            }
            SystolicState::Computing => {
                let max_cycle = (16 * 2 + 16 - 2) as f32;
                let cur_cycle = soc.systolic.cycle as f32;
                40.0 + (cur_cycle / max_cycle) * 40.0
            }
            SystolicState::Storing => {
                let words_stored = match soc.systolic.dma.state {
                    DmaState::StoringC { row, col } => row * 16 + col,
                    DmaState::Done => 256,
                    _ => 0,
                };
                80.0 + (words_stored as f32 / 256.0) * 20.0
            }
            SystolicState::Done => 100.0,
        };

        let systolic_progress = progress_f32.clamp(0.0, 100.0) as u16;

        // --- 6. Systolic Grids ---
        let mut systolic_grid = Vec::with_capacity(ARRAY_SIZE);
        let mut sram_a_grid = Vec::with_capacity(ARRAY_SIZE);
        let mut sram_b_grid = Vec::with_capacity(ARRAY_SIZE);

        for i in 0..ARRAY_SIZE {
            let mut pe_row = String::with_capacity(INNER_DIM * 2);
            let mut a_row = String::with_capacity(INNER_DIM * 2);
            let mut b_row = String::with_capacity(INNER_DIM * 2);

            for j in 0..INNER_DIM {
                if soc.systolic.pes[i][j].is_active
                    && soc.systolic.state == SystolicState::Computing
                {
                    pe_row.push_str("■ ");
                } else {
                    pe_row.push_str("□ ");
                }

                if soc.systolic.scratchpad.a_valid[i][j] {
                    a_row.push_str("■ ");
                } else {
                    a_row.push_str("□ ");
                }

                if soc.systolic.scratchpad.b_valid[i][j] {
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
            cycle: soc.cycle,
            pc: soc.cpu.pc,
            pipeline,
            registers,
            i_cache_state: i_cache_state_str,
            d_cache_state: d_cache_state_str,
            bus_state: bus_state_str,
            bus_owner: bus_owner_str,
            systolic_state: systolic_state_str,
            systolic_progress,
            systolic_grid,
            sram_a_grid,
            sram_b_grid,
        }
    }
}
