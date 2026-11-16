// /*
//     alu.rs
//     - 이 파일은 CPU의 산술 논리 연산 장치(ALU)를 정의합니다.
//     - ALU는 두 개의 32비트 입력(a, b)과 ALU 제어 신호를 받아서 연산을 수행하고 결과를 반환합니다.
//     - ALU 제어 신호는 opcode, funct3, funct7 필드에 따라 결정되며, 다양한 산술 및 논리 연산을 지원합니다.
//     - Branch 명령어의 경우, ALU는 비교 연산을 수행하여 조건 분기 여부를 결정합니다.
// */
mod decode;
mod divider;
mod integer;
mod multiplier;
mod shifter;
mod types;

#[cfg(test)]
mod tests;

use crate::hardware::clocked::Clocked;

use divider::{DividerState, DividerUnit};
use integer::{IntegerState, IntegerUnit};
use multiplier::{MultiplierState, MultiplierUnit};
use shifter::{ShifterState, ShifterUnit};

use types::{UNIT_COUNT, UnitKind};

pub use types::{
    AluConfig, AluDecodeError, AluInputs, AluOperation, AluOutputs, AluRequest, AluResult,
};

// ============================================================================
// 디버깅·테스트용 공통 상태 조회 형식
// ============================================================================

/// 기존 states() 인터페이스를 유지하기 위한 조회용 타입.
///
/// 실제 제어 상태는 각 실행 유닛이 소유한다.
/// 이 타입으로 실행 상태를 별도 저장하지 않는다.

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitState {
    Idle,
    Running { cycles_left: u8, request: AluRequest },
    ResultReady { result: AluResult },
}

// ============================================================================
// 상위 ALU
// ============================================================================

pub struct Alu {
    integer: IntegerUnit,
    multiplier: MultiplierUnit,
    divider: DividerUnit,
    shifter: ShifterUnit,

    // 전체 ALU에서 미완료 요청 하나만 허용한다.
    active_unit: Option<UnitKind>,
}

pub struct AluNextState {
    integer: IntegerState,
    multiplier: MultiplierState,
    divider: DividerState,
    shifter: ShifterState,

    active_unit: Option<UnitKind>,
}

impl Alu {
    pub fn new() -> Self {
        Self::with_config(AluConfig::default())
    }

    pub fn with_config(config: AluConfig) -> Self {
        Self {
            integer: IntegerUnit::new(config.integer_cycles),
            multiplier: MultiplierUnit::new(config.multiply_cycles),
            divider: DividerUnit::new(config.divide_cycles),
            shifter: ShifterUnit::new(config.shift_cycles),

            active_unit: None,
        }
    }

    pub fn is_idle(&self) -> bool {
        self.active_unit.is_none()
    }

    /// 배열 순서:
    /// Integer, Multiply, Divide, Shift
    pub fn states(&self) -> [UnitState; UNIT_COUNT] {
        let integer = match self.integer.state() {
            IntegerState::Idle => UnitState::Idle,

            IntegerState::Running {
                cycles_left,
                request,
            } => UnitState::Running {
                cycles_left,
                request,
            },

            IntegerState::ResultReady { result } => UnitState::ResultReady { result },
        };

        let multiplier = match self.multiplier.state() {
            MultiplierState::Idle => UnitState::Idle,

            MultiplierState::Running {
                cycles_left,
                request,
            } => UnitState::Running {
                cycles_left,
                request,
            },

            MultiplierState::ResultReady { result } => UnitState::ResultReady { result },
        };

        let divider = match self.divider.state() {
            DividerState::Idle => UnitState::Idle,

            DividerState::Running {
                cycles_left,
                request,
            } => UnitState::Running {
                cycles_left,
                request,
            },

            DividerState::ResultReady { result } => UnitState::ResultReady { result },
        };

        let shifter = match self.shifter.state() {
            ShifterState::Idle => UnitState::Idle,

            ShifterState::Running {
                cycles_left,
                request,
            } => UnitState::Running {
                cycles_left,
                request,
            },

            ShifterState::ResultReady { result } => UnitState::ResultReady { result },
        };

        [integer, multiplier, divider, shifter]
    }
}

impl Clocked for Alu {
    type Inputs = AluInputs;
    type Outputs = AluOutputs;
    type NextState = AluNextState;

    fn evaluate(&self, inputs: &Self::Inputs) -> (Self::Outputs, Self::NextState) {
        // 진행 중인 유닛을 우선 선택한다.
        // 유휴 상태에서만 새 요청으로 유닛을 선택한다.
        let selected = if inputs.flush {
            None
        } else {
            self.active_unit
                .or_else(|| inputs.request.map(|request| request.operation.unit()))
        };

        let inputs_for = |kind: UnitKind| AluInputs {
            request: if !inputs.flush && self.active_unit.is_none() && selected == Some(kind) {
                inputs.request
            } else {
                None
            },

            result_ready: selected == Some(kind) && inputs.result_ready,

            flush: inputs.flush,
        };

        // 하위 유닛들은 현재 상태를 읽어 다음 상태만 계산한다.
        let (integer_out, integer_next) = self.integer.evaluate(&inputs_for(UnitKind::Integer));

        let (multiplier_out, multiplier_next) =
            self.multiplier.evaluate(&inputs_for(UnitKind::Multiply));

        let (divider_out, divider_next) = self.divider.evaluate(&inputs_for(UnitKind::Divide));

        let (shifter_out, shifter_next) = self.shifter.evaluate(&inputs_for(UnitKind::Shift));

        // UnitKind::index()와 같은 순서.
        let unit_outputs = [integer_out, multiplier_out, divider_out, shifter_out];

        let next_is_idle = [
            integer_next.is_idle(),
            multiplier_next.is_idle(),
            divider_next.is_idle(),
            shifter_next.is_idle(),
        ];

        let outputs = if inputs.flush {
            AluOutputs::default()
        } else {
            let mut outputs = match selected {
                Some(kind) => unit_outputs[kind.index()],

                None => AluOutputs {
                    request_ready: true,
                    result: None,
                },
            };

            // 기존 작업의 결과를 수락하는 사이클에는
            // 새 요청을 동시에 수락하지 않는다.
            //
            // 유휴 상태에서 시작하는 지연시간 1의 연산은
            // 요청과 결과 전달이 같은 사이클에 가능하다.
            outputs.request_ready &= self.active_unit.is_none();

            outputs
        };

        let active_unit = if inputs.flush {
            None
        } else {
            selected.filter(|kind| !next_is_idle[kind.index()])
        };

        (
            outputs,
            AluNextState {
                integer: integer_next,
                multiplier: multiplier_next,
                divider: divider_next,
                shifter: shifter_next,

                active_unit,
            },
        )
    }

    fn commit(&mut self, next: Self::NextState) {
        self.integer.commit(next.integer);
        self.multiplier.commit(next.multiplier);
        self.divider.commit(next.divider);
        self.shifter.commit(next.shifter);

        self.active_unit = next.active_unit;
    }
}

impl Default for Alu {
    fn default() -> Self {
        Self::new()
    }
}

// use crate::hardware::soc::types::OpStatus;

// // 고유한 ALU 제어 신호 정의
// #[rustfmt::skip]
// #[allow (clippy::all)]
// enum AluControl {
//     ADD, SUB, AND, OR, XOR,
//     SLL, SRL, SRA, SLT, SLTU,
//     MUL, MULH, MULHSU, MULHU,
//     DIV, DIVU, REM, REMU, LUI,
// }

// // ALU 제어 신호에 따른 연산 지연 시간 정의
// impl AluControl {
//     pub fn latency(&self) -> u8 {
//         match self {
//             AluControl::MUL | AluControl::MULH | AluControl::MULHSU | AluControl::MULHU => 3, // 곱셈 연산은 3 사이클
//             AluControl::DIV | AluControl::DIVU | AluControl::REM | AluControl::REMU => 20, // 나눗셈 연산은 20 사이클(가정)
//             _ => 1, // 나머지 연산은 1 사이클
//         }
//     }
// }

// // 다중 사이클 추적을 위한 상태 머신 Enum
// #[derive(Clone, Copy, PartialEq)]
// pub enum MultiCycleState {
//     Ready,
//     Processing {
//         cycles_left: u8, // 남은 사이클 수
//         lattched_a: u32, // 연산에 사용될 첫 번째 입력값
//         lattched_b: u32, // 연산에 사용될 두 번째 입력값
//     },
// }

// pub struct Alu {
//     pub state: MultiCycleState, // ALU의 현재 상태
// }

// impl Alu {
//     pub fn new() -> Self {
//         Alu {
//             state: MultiCycleState::Ready,
//         }
//     }

//     fn alu_control(&self, alu_op: u8, funct3: u8, funct7: u8) -> AluControl {
//         let inst30 = (funct7 >> 5) & 0x1; // funct7의 30번째 비트 추출

//         match alu_op {
//             0b00 => AluControl::ADD, // 메모리참조 명령어
//             0b01 => match funct3 {
//                 0x0 | 0x1 => AluControl::SUB,  // BEQ, BNE -> SUB 결과가 0인지 체크
//                 0x4 | 0x5 => AluControl::SLT,  // BLT, BGE -> SLT 결과(1 or 0) 체크
//                 0x6 | 0x7 => AluControl::SLTU, // BLTU, BGEU -> SLTU 결과(1 or 0) 체크
//                 _ => panic!("Unsupported Branch instruction"),
//             },
//             0b10 => match (funct3, funct7) {
//                 // R-Type 명령어
//                 (0x0, 0x00) => AluControl::ADD, // ADD
//                 (0x0, 0x20) => AluControl::SUB, // SUB
//                 (0x4, 0x00) => AluControl::XOR, // XOR
//                 (0x6, 0x00) => AluControl::OR,  // OR
//                 (0x7, 0x00) => AluControl::AND, // AND

//                 (0x1, 0x00) => AluControl::SLL,  // SLL
//                 (0x2, 0x00) => AluControl::SLT,  // SLT
//                 (0x3, 0x00) => AluControl::SLTU, // SLTU
//                 (0x5, 0x00) => AluControl::SRL,  // SRL
//                 (0x5, 0x20) => AluControl::SRA,  // SRA

//                 (0x0, 0x01) => AluControl::MUL,    // MUL
//                 (0x1, 0x01) => AluControl::MULH,   // MULH
//                 (0x2, 0x01) => AluControl::MULHSU, // MULHSU
//                 (0x3, 0x01) => AluControl::MULHU,  // MULHU
//                 (0x4, 0x01) => AluControl::DIV,    // DIV
//                 (0x5, 0x01) => AluControl::DIVU,   // DIVU
//                 (0x6, 0x01) => AluControl::REM,    // REM
//                 (0x7, 0x01) => AluControl::REMU,   // REMU

//                 _ => panic!("Unsupported Shift instruction"),
//             },
//             0b11 => match funct3 {
//                 // I-Type 명령어
//                 0x0 => AluControl::ADD, // ADDI
//                 0x4 => AluControl::XOR, // XORI
//                 0x6 => AluControl::OR,  // ORI
//                 0x7 => AluControl::AND, // ANDI

//                 0x1 => AluControl::SLL,  // SLLI
//                 0x2 => AluControl::SLT,  // SLTI
//                 0x3 => AluControl::SLTU, // SLTIU
//                 0x5 => {
//                     match inst30 {
//                         0 => AluControl::SRL, // SRLI
//                         1 => AluControl::SRA, // SRAI
//                         _ => panic!("Unsupported Shift instruction"),
//                     }
//                 }
//                 _ => panic!("Unsupported I-Type instruction"),
//             },
//             0b100 => AluControl::LUI, // LUI 명령어

//             _ => panic!("Unsupported ALU Op"),
//         }
//     }

//     #[rustfmt::skip] // match arms를 정렬하지 않음
//     fn execute(&self, a: u32, b: u32, alu_control_signal: AluControl) -> u32 {
//         let shamt = b & 0x1F; // RISC-V 시프트 량은 하위 5비트만 사용

//         match alu_control_signal {
//             // 산술 및 논리 연산
//             AluControl::ADD => a.wrapping_add(b),
//             AluControl::SUB => a.wrapping_sub(b),
//             AluControl::XOR => a ^ b,
//             AluControl::OR => a | b,
//             AluControl::AND => a & b,

//             // Shift 연산
//             AluControl::SLL => a << shamt,
//             AluControl::SRL => a >> shamt,
//             AluControl::SRA => ((a as i32) >> shamt) as u32, // 산술 시프트
//             AluControl::SLT => if (a as i32) < (b as i32) { 1 } else { 0 },
//             AluControl::SLTU => if a < b { 1 } else { 0 },

//              // 곱셈/나눗셈 연산
//             AluControl::MUL => a.wrapping_mul(b), // 32비트 곱셈 (하위 32비트 결과)
//             AluControl::MULH => {
//                 let a_i64 = a as i32 as i64;
//                 let b_i64 = b as i32 as i64;
//                 (a_i64.wrapping_mul(b_i64) >> 32) as u32
//             }
//             AluControl::MULHSU => {
//                 let a_i64 = a as i32 as i64;
//                 let b_u64 = b as u64 as i64; // Zero-extended u32 -> u64 -> i64
//                 (a_i64.wrapping_mul(b_u64) >> 32) as u32
//             }
//             AluControl::MULHU => {
//                 let a_u64 = a as u64;
//                 let b_u64 = b as u64;
//                 (a_u64.wrapping_mul(b_u64) >> 32) as u32
//             }
//             AluControl::DIV => {
//                 let a_i32 = a as i32;
//                 let b_i32 = b as i32;
//                 if b_i32 == 0 {
//                     0xFFFFFFFF // Division by zero 예외
//                 } else if a_i32 == i32::MIN && b_i32 == -1 {
//                     i32::MIN as u32 // Overflow 예외 (Signed Overflow)
//                 } else {
//                     a_i32.wrapping_div(b_i32) as u32
//                 }
//             }
//             AluControl::DIVU => match b {
//                 0 => 0xFFFFFFFF, // Division by zero 예외
//                 _ => a.wrapping_div(b),
//             },
//             AluControl::REM => {
//                 let a_i32 = a as i32;
//                 let b_i32 = b as i32;
//                 if b_i32 == 0 {
//                     a // Division by zero 예외 (나머지는 피제수 그대로 반환)
//                 } else if a_i32 == i32::MIN && b_i32 == -1 {
//                     0 // Overflow 예외
//                 } else {
//                     a_i32.wrapping_rem(b_i32) as u32
//                 }
//             }
//             AluControl::REMU => match b {
//                 0 => a, // Division by zero 예외 (나머지는 피제수 그대로 반환)
//                 _ => a.wrapping_rem(b),
//             },
//             AluControl::LUI => b, // LUI 명령어는 immediate 값을 그대로 반환
//         }
//     }

//     pub fn execute_with_cycles(
//         &mut self,
//         a: u32,
//         b: u32,
//         alu_op: u8,
//         funct3: u8,
//         funct7: u8,
//     ) -> OpStatus<(u32, bool)> {
//         let alu_control_signal = self.alu_control(alu_op, funct3, funct7);
//         let latency = alu_control_signal.latency();

//         match self.state {
//             MultiCycleState::Ready => {
//                 if latency > 1 {
//                     self.state = MultiCycleState::Processing {
//                         cycles_left: latency - 1,
//                         lattched_a: a,
//                         lattched_b: b,
//                     };
//                     return OpStatus::Busy;
//                 }
//             }
//             MultiCycleState::Processing {
//                 cycles_left,
//                 lattched_a,
//                 lattched_b,
//             } => {
//                 if cycles_left > 1 {
//                     self.state = MultiCycleState::Processing {
//                         cycles_left: cycles_left - 1,
//                         lattched_a,
//                         lattched_b,
//                     };
//                     return OpStatus::Busy;
//                 } else {
//                     self.state = MultiCycleState::Ready;

//                     // 계속 바뀌는 a, b 대신 latched_a, latched_b를 사용하여 연산 수행
//                     let alu_out = self.execute(lattched_a, lattched_b, alu_control_signal);
//                     return OpStatus::Complete((alu_out, alu_out == 0));
//                 }
//             }
//         }

//         let alu_out = self.execute(a, b, alu_control_signal);
//         OpStatus::Complete((alu_out, alu_out == 0))
//     }
// }

// impl Default for Alu {
//     fn default() -> Self {
//         Self::new()
//     }
// }
