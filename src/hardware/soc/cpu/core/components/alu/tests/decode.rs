use super::super::AluOperation;

#[test]
fn decodes_integer_and_m_extension() {
    assert_eq!(
        AluOperation::decode(0b010, 0x0, 0x00),
        Ok(AluOperation::Add),
    );

    assert_eq!(
        AluOperation::decode(0b010, 0x0, 0x20),
        Ok(AluOperation::Sub),
    );

    assert_eq!(
        AluOperation::decode(0b010, 0x0, 0x01),
        Ok(AluOperation::Mul),
    );

    assert_eq!(
        AluOperation::decode(0b010, 0x4, 0x01),
        Ok(AluOperation::Div),
    );
}

#[test]
fn immediate_arithmetic_does_not_check_funct7() {
    // 일반 즉시값 명령의 상위 비트는 즉시값의 일부다.
    assert_eq!(
        AluOperation::decode(0b011, 0x0, 0x7F),
        Ok(AluOperation::Add),
    );
}

#[test]
fn validates_shift_immediate_encoding() {
    assert_eq!(
        AluOperation::decode(0b011, 0x1, 0x00),
        Ok(AluOperation::Sll),
    );

    assert_eq!(
        AluOperation::decode(0b011, 0x5, 0x20),
        Ok(AluOperation::Sra),
    );

    assert!(AluOperation::decode(0b011, 0x1, 0x20).is_err());
}

#[test]
fn rejects_unsupported_control_signals() {
    assert!(AluOperation::decode(0b111, 0x0, 0x00).is_err());

    assert!(AluOperation::decode(0b010, 0x0, 0x7F).is_err());
}
