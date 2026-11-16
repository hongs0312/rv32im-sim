/// 클럭 기반 부품의 공통 계약.
///
/// evaluate:
///   현재 상태를 변경하지 않고 출력과 다음 상태를 계산한다.
///
/// commit:
///   계산된 다음 상태를 클럭 경계에서 반영한다.
pub trait Clocked {
    type Inputs;
    type Outputs;
    type NextState;

    fn evaluate(&self, inputs: &Self::Inputs) -> (Self::Outputs, Self::NextState);

    fn commit(&mut self, next: Self::NextState);
}
