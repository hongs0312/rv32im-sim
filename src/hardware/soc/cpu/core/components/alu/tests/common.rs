use crate::hardware::clocked::Clocked;

use super::super::{AluInputs, AluOperation, AluOutputs, AluRequest};

/// 테스트용 요청 생성.
pub(super) fn request(operation: AluOperation, a: u32, b: u32) -> AluRequest {
    AluRequest { operation, a, b }
}

/// ALU 또는 하위 실행 유닛의 한 사이클 실행.
///
/// 독립된 부품 테스트에서만 사용한다.
/// 전체 시스템의 부품별 commit 순서를 정의하는 함수는 아니다.
pub(super) fn cycle<C>(component: &mut C, inputs: AluInputs) -> AluOutputs
where
    C: Clocked<Inputs = AluInputs, Outputs = AluOutputs>,
{
    let (outputs, next) = component.evaluate(&inputs);
    component.commit(next);
    outputs
}
