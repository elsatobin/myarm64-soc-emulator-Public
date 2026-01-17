use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmulatorError {
    #[error("cpu error: {0}")]
    Cpu(#[from] CpuError),

    #[error("memory error: {0}")]
    Memory(#[from] MemoryError),

    #[error("bus error: {0}")]
    Bus(#[from] BusError),

    #[error("peripheral error: {0}")]
    Peripheral(#[from] PeripheralError),

    #[error("configuration error: {message}")]
    Config { message: String },
}

#[derive(Debug, Error)]
pub enum CpuError {
    #[error("invalid instruction at address {address:#x}: {reason}")]
    InvalidInstruction { address: u64, reason: String },

    #[error("undefined instruction at address {address:#x}")]
    UndefinedInstruction { address: u64 },

    #[error("invalid register access: {0}")]
    InvalidRegister(String),

    #[error("exception: {0}")]
    Exception(String),
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("address {address:#x} out of bounds (size: {size:#x})")]
    OutOfBounds { address: u64, size: u64 },

    #[error("unaligned access at address {address:#x} for size {access_size}")]
    UnalignedAccess { address: u64, access_size: usize },

    #[error("page fault at address {address:#x}")]
    PageFault { address: u64 },

    #[error("permission denied: {0}")]
    PermissionDenied(String),
}

#[derive(Debug, Error)]
pub enum BusError {
    #[error("no device mapped at address {address:#x}")]
    UnmappedAddress { address: u64 },

    #[error("address range {start:#x}-{end:#x} overlaps with existing mapping")]
    OverlappingRange { start: u64, end: u64 },

    #[error("bus timeout accessing address {address:#x}")]
    Timeout { address: u64 },
}

#[derive(Debug, Error)]
pub enum PeripheralError {
    #[error("peripheral '{name}' not found")]
    NotFound { name: String },

    #[error("invalid register offset {offset:#x} for peripheral '{name}'")]
    InvalidRegister { name: String, offset: u64 },

    #[error("peripheral '{name}' error: {message}")]
    Device { name: String, message: String },
}

pub type Result<T> = std::result::Result<T, EmulatorError>;
