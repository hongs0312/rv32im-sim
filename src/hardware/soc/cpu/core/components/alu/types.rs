//! ALU 공통 타입.
//!
//! 포함:
//! - 연산 종류
//! - 실행 유닛 분류
//! - 지연시간 설정
//! - 요청·응답 인터페이스
//!
//! 포함하지 않음:
//! - 연산 디코드 구현
//! - 산술 계산
//! - 실행 유닛 상태 머신
//! - 다음 상태 반영

// ============================================================================
// 실행 유닛 분류
// ============================================================================

/// ALU 내부 실행 유닛 개수.
pub(super) const UNIT_COUNT: usize = 4;

/// ALU 내부 요청 분배에 사용하는 실행 유닛 종류.
///
/// 외부 Core에서는 이 분류를 알 필요가 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnitKind {
    Integer,
    Multiply,
    Divide,
    Shift,
}

impl UnitKind {
    /// 기존 배열 기반 실행 유닛 배치와 대응한다.
    pub(super) const fn index(self) -> usize {
        match self {
            Self::Integer => 0,
            Self::Multiply => 1,
            Self::Divide => 2,
            Self::Shift => 3,
        }
    }
}

// ============================================================================
// 연산 종류
// ============================================================================
#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluOperation {
    Add, Sub,                   // 덧셈·뺄셈
    And, Or, Xor,               // 논리 연산
    Sll, Srl, Sra,              // 시프트
    Slt, Sltu,                  // 비교
    Mul, Mulh, Mulhsu, Mulhu,   // 곱셈
    Div, Divu, Rem, Remu,       // 나눗셈·나머지
    Lui,                        // 즉시값 전달
}

impl AluOperation {
    /// 해당 연산을 처리할 실행 유닛.
    pub(super) const fn unit(self) -> UnitKind {
        match self {
            Self::Mul | Self::Mulh | Self::Mulhsu | Self::Mulhu => UnitKind::Multiply,
            Self::Div | Self::Divu | Self::Rem | Self::Remu => UnitKind::Divide,
            Self::Sll | Self::Srl | Self::Sra => UnitKind::Shift,
            Self::Add | Self::Sub | Self::And => UnitKind::Integer,
            Self::Or | Self::Xor => UnitKind::Integer,
            Self::Slt | Self::Sltu | Self::Lui => UnitKind::Integer,
        }
    }
}

/// ALU 제어 신호를 지원하는 연산으로 변환하지 못했을 때의 정보.
///
/// 전체 명령어의 유효성·특권 검사는 명령 디코더와 Core가 담당한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluDecodeError {
    pub alu_op: u8,
    pub funct3: u8,
    pub funct7: u8,
}

// ============================================================================
// 지연시간 설정
// ============================================================================

/// 요청 수락 사이클을 포함한 실행 사이클 수.
///
/// - 1사이클: 요청 수락 사이클에 결과 제공 가능.
/// - 3사이클: N에서 수락하면 N+2에 결과 제공 가능.
///
/// 결과가 준비된 후 수락을 기다리는 시간은 포함하지 않는다.
/// 각 값은 1 이상이어야 하며, 생성 시 실행 유닛에서 검사한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluConfig {
    pub integer_cycles: u8,
    pub multiply_cycles: u8,
    pub divide_cycles: u8,
    pub shift_cycles: u8,
}

impl Default for AluConfig {
    fn default() -> Self {
        // 현재 ALU 설정 유지.
        Self {
            integer_cycles: 1,
            multiply_cycles: 3,
            divide_cycles: 20,
            shift_cycles: 1,
        }
    }
}

// ============================================================================
// 요청·결과 데이터
// ============================================================================

/// ALU에 제출하는 요청.
///
/// 요청 수락 시 실행에 필요한 정보를 내부에 보관한다.
/// 현재 인터페이스는 ALU 전체에서 미완료 요청 하나만 허용한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluRequest {
    pub operation: AluOperation,
    pub a: u32,
    pub b: u32,
}

/// ALU 계산 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AluResult {
    pub value: u32,

    /// 결과값이 0이면 true.
    ///
    /// 분기 성립 여부 자체가 아니라 ALU 결과의 영 여부다.
    pub zero: bool,
}

impl AluResult {
    /// 결과값과 zero 플래그를 함께 생성한다.
    pub const fn new(value: u32) -> Self {
        Self {
            value,
            zero: value == 0,
        }
    }
}

// ============================================================================
// 사이클 입력·출력
// ============================================================================

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AluInputs {
    /// Some이면 요청이 유효하다.
    ///
    /// request_ready=true일 때만 요청이 수락된다.
    /// 수락 전까지 요청자는 같은 요청을 유지해야 한다.
    pub request: Option<AluRequest>,

    /// 다음 단계가 이번 사이클의 결과를 수락할 수 있다.
    pub result_ready: bool,

    /// 현재 작업 및 완료 대기 결과를 취소한다.
    ///
    /// flush는 요청·결과 전달보다 우선하며,
    /// 해당 사이클에는 어떤 전달도 성립하지 않는다.
    pub flush: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AluOutputs {
    /// 이번 사이클에 새 요청을 수락할 수 있다.
    pub request_ready: bool,

    /// Some이면 결과가 유효하다.
    ///
    /// result_ready=false이면 같은 결과를 유지해야 한다.
    /// 단, flush로 취소되는 경우는 제외한다.
    pub result: Option<AluResult>,
}

impl AluOutputs {
    /// 이번 사이클의 요청 전달 성립 여부.
    pub fn request_fire(&self, inputs: &AluInputs) -> bool {
        !inputs.flush && self.request_ready && inputs.request.is_some()
    }

    /// 이번 사이클의 결과 전달 성립 여부.
    pub fn result_fire(&self, inputs: &AluInputs) -> bool {
        !inputs.flush && self.result.is_some() && inputs.result_ready
    }
}
