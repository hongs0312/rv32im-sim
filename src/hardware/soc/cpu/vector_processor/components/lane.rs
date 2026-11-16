use std::convert::TryInto;

// 💡 요소의 크기(Standard Element Width)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sew {
    E8 = 1,  // 8-bit
    E16 = 2, // 16-bit
    E32 = 4, // 32-bit
}

// 💡 벡터 연산 종류 (필요에 따라 계속 추가)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorOp {
    Add,
    Sub,
    Mul,
    And,
    Or,
    Xor,
}

// -----------------------------------------------------
// 1. 단일 차선 (32비트 처리기)
// -----------------------------------------------------
#[derive(Clone, Copy)]
pub struct VectorLane;

impl VectorLane {
    pub fn new() -> Self {
        Self
    }

    /// 32비트(4바이트) 데이터를 SEW에 맞게 쪼개서 연산합니다.
    pub fn compute(&self, op: VectorOp, src1: [u8; 4], src2: [u8; 4], sew: Sew) -> [u8; 4] {
        let mut res = [0u8; 4];

        match sew {
            Sew::E8 => {
                for i in 0..4 {
                    res[i] = match op {
                        VectorOp::Add => src1[i].wrapping_add(src2[i]),
                        VectorOp::Sub => src1[i].wrapping_sub(src2[i]),
                        VectorOp::Mul => src1[i].wrapping_mul(src2[i]),
                        VectorOp::And => src1[i] & src2[i],
                        VectorOp::Or => src1[i] | src2[i],
                        VectorOp::Xor => src1[i] ^ src2[i],
                    };
                }
            }
            Sew::E16 => {
                for i in (0..4).step_by(2) {
                    let a = u16::from_le_bytes([src1[i], src1[i + 1]]);
                    let b = u16::from_le_bytes([src2[i], src2[i + 1]]);
                    let r = match op {
                        VectorOp::Add => a.wrapping_add(b),
                        VectorOp::Sub => a.wrapping_sub(b),
                        VectorOp::Mul => a.wrapping_mul(b),
                        VectorOp::And => a & b,
                        VectorOp::Or => a | b,
                        VectorOp::Xor => a ^ b,
                    };
                    res[i..i + 2].copy_from_slice(&r.to_le_bytes());
                }
            }
            Sew::E32 => {
                let a = u32::from_le_bytes(src1);
                let b = u32::from_le_bytes(src2);
                let r = match op {
                    VectorOp::Add => a.wrapping_add(b),
                    VectorOp::Sub => a.wrapping_sub(b),
                    VectorOp::Mul => a.wrapping_mul(b),
                    VectorOp::And => a & b,
                    VectorOp::Or => a | b,
                    VectorOp::Xor => a ^ b,
                };
                res = r.to_le_bytes();
            }
        }
        res
    }
}

// -----------------------------------------------------
// 2. 128비트 벡터 실행 유닛 (V-EX)
// -----------------------------------------------------
pub struct VectorUnit {
    lanes: [VectorLane; 4], // 32비트 레인 4개를 병렬로 배치!
}

impl VectorUnit {
    pub fn new() -> Self {
        Self {
            lanes: [VectorLane::new(); 4],
        }
    }

    /// 128비트(16바이트) 데이터를 4개의 차선에 분배하여 연산합니다.
    pub fn execute(&self, op: VectorOp, vs1: [u8; 16], vs2: [u8; 16], sew: Sew) -> [u8; 16] {
        let mut vd = [0u8; 16];

        // 하드웨어 상으로는 4개 차선이 '동시에' 가동되는 것을 for문으로 모사
        for i in 0..4 {
            let chunk1: [u8; 4] = vs1[i * 4..(i + 1) * 4].try_into().unwrap();
            let chunk2: [u8; 4] = vs2[i * 4..(i + 1) * 4].try_into().unwrap();

            // 각 차선에 4바이트씩 할당하여 연산
            let res = self.lanes[i].compute(op, chunk1, chunk2, sew);

            // 결과를 다시 128비트로 조립
            vd[i * 4..(i + 1) * 4].copy_from_slice(&res);
        }

        vd
    }
}
