use super::ControlSignals;

pub struct ControlUnit;

impl ControlUnit {
    pub fn decode(opcode: u8, funct3: u8, funct7: u8) -> ControlSignals {
        // 기본값으로 ControlSignals를 초기화
        let mut control = ControlSignals::new(opcode, funct3, funct7);

        match opcode {
            0x37 => {
                // LUI
                control.alu_src_b = true;
                control.alu_op = 0b100; // LUI Pass-through
                control.reg_write = true;
                control.wb_src = false;
            }
            0x17 => {
                // AUIPC
                control.alu_src_a = true; // PC 선택
                control.alu_src_b = true; // Imm 선택
                control.alu_op = 0b00; // ADD 수행 (PC + Imm)
                control.reg_write = true;
                control.wb_src = false;
            }
            0x33 => {
                // R-Type
                control.alu_src_a = false;
                control.alu_src_b = false;
                control.alu_op = 0b10;
                control.reg_write = true;
                control.wb_src = false;
            }
            0x13 => {
                // I-Type ALU
                control.alu_src_a = false;
                control.alu_src_b = true;
                control.alu_op = 0b11;
                control.reg_write = true;
                control.wb_src = false;
            }
            0x03 => {
                // Load
                control.alu_src_a = false;
                control.alu_src_b = true;
                control.alu_op = 0b00; // ADD (Addr = rs1 + Imm)
                control.mem_read = true;
                control.reg_write = true;
                control.wb_src = true; // Memory 읽기 값 선택
            }
            0x23 => {
                // Store
                control.alu_src_a = false;
                control.alu_src_b = true;
                control.alu_op = 0b00; // ADD (Addr = rs1 + Imm)
                control.mem_write = true;
            }
            0x63 => {
                // Branch
                control.alu_src_a = false;
                control.alu_src_b = false;
                control.alu_op = 0b01; // Branch Compare
                control.branch = true;
            }
            0x6F => {
                // JAL
                control.alu_src_a = true; // PC 선택
                control.alu_src_b = true; // Imm 선택 (Target = PC + Imm)
                control.alu_op = 0b00; // ADD
                control.jump = true; // JAL/JALR 공통 Jump 신호
                control.reg_write = true;
                control.wb_src = false; // PC + 4 선택!
            }
            0x67 => {
                // JALR
                control.alu_src_a = false; // rs1 선택! (Target = rs1 + Imm)
                control.alu_src_b = true; // Imm 선택
                control.alu_op = 0b00; // ADD
                control.jump = true; // JAL/JALR 공통 Jump 신호
                control.is_jalr = true; // LSB Masking(0bit Clear) 처리를 위해 표시
                control.reg_write = true;
                control.wb_src = false; // PC + 4 선택!
            }
            0x73 => {
                match funct3 {
                    // 유효한 CSR 명령 형식
                    // 레지스터: CSRRW / CSRRS / CSRRC
                    // 즉시값:   CSRRWI / CSRRSI / CSRRCI
                    0b001 | 0b010 | 0b011 | 0b101 | 0b110 | 0b111 => {
                        control.is_csr = true;

                        // 비트 2는 레지스터/즉시값 형식 구분에 사용한다.
                        control.csr_op = funct3;

                        control.reg_write = true;
                        control.wb_src = false;
                    }

                    0b000 => {
                        // 기존 SYSTEM 판별을 유지한다.
                        // 전체 명령어 기반의 엄밀한 판별은 별도 수정 대상이다.
                        match funct7 {
                            0x00 => control.is_ecall = true,
                            0x08 => control.is_sret = true,
                            0x09 => control.is_fence = true,
                            0x18 => control.is_mret = true,
                            _ => {}
                        }
                    }

                    0b100 => {
                        // 예약된 형식.
                        // EX 진입부에서 illegal-instruction 예외로 처리한다.
                    }

                    _ => unreachable!("funct3는 3비트 값이어야 합니다."),
                }
            }
            0x0F => {
                control.is_fence = true;

                // if funct3 == 0x1 {
                //     // fence.i (명령어 캐시 동기화)
                //     control.is_fence_i = true;
                // }

                // fence는 메모리에 데이터를 쓰거나(MemWrite),
                // 레지스터에 값을 기록(RegWrite)하지 않으므로 나머지 제어 신호는 false로 유지해야 합니다.
                control.mem_read = false;
                control.mem_write = false;
                control.reg_write = false;
            }

            _ => {}
        }

        control
    }
}
