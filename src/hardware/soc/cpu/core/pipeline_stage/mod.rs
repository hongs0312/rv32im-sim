pub mod execute;
pub mod instruction_decode;
pub mod instruction_fetch;
pub mod memory_access;
pub mod write_back;

// 기존 단계 코드의 `use super::IfIdRegister` 등을 유지한다.
pub use super::pipeline_registers::{ExMemRegister, IdExRegister, IfIdRegister, MemWbRegister};
