use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct ViqPacket {
    pub pc: u32,
    pub instruction: u32, // 벡터 명령어 원본

    // 스칼라 연산자가 필요한 경우 원본 값을 복사해서 전달
    pub rs1_val: u32,
    pub rs2_val: u32,
}

pub struct VectorInstructionQueue {
    queue: VecDeque<ViqPacket>,
    capacity: usize,
}

impl VectorInstructionQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn peek(&self) -> Option<ViqPacket> {
        self.queue.front().copied()
    }

    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn push(&mut self, pecket: ViqPacket) {
        if !self.is_full() {
            self.queue.push_back(pecket);
        }
    }

    pub fn pop(&mut self) -> Option<ViqPacket> {
        self.queue.pop_front()
    }
}
