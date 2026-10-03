#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpStatus<T = ()> {
    Busy,
    Complete(T),
}
