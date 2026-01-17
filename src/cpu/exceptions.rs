#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ExceptionLevel {
    El0 = 0,
    El1 = 1,
    El2 = 2,
    El3 = 3,
}

impl ExceptionLevel {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::El0),
            1 => Some(Self::El1),
            2 => Some(Self::El2),
            3 => Some(Self::El3),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exception {
    Synchronous,
    Irq,
    Fiq,
    SError,
}

impl Exception {
    pub fn vector_offset(&self, from_lower_el: bool, is_aarch64: bool) -> u64 {
        let base = if from_lower_el { if is_aarch64 { 0x400 } else { 0x600 } } else { 0x200 };

        base + match self {
            Self::Synchronous => 0x000,
            Self::Irq => 0x080,
            Self::Fiq => 0x100,
            Self::SError => 0x180,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExceptionSyndrome {
    pub exception_class: u8,
    pub instruction_length_32: bool,
    pub iss: u32,
}

impl ExceptionSyndrome {
    pub fn to_esr(&self) -> u32 {
        let mut esr = (self.exception_class as u32 & 0x3f) << 26;
        if self.instruction_length_32 {
            esr |= 1 << 25;
        }
        esr |= self.iss & 0x1ffffff;
        esr
    }

    pub fn from_esr(esr: u32) -> Self {
        Self {
            exception_class: ((esr >> 26) & 0x3f) as u8,
            instruction_length_32: (esr & (1 << 25)) != 0,
            iss: esr & 0x1ffffff,
        }
    }
}

pub mod exception_class {
    pub const UNKNOWN: u8 = 0x00;
    pub const WFI_WFE: u8 = 0x01;
    pub const SVC_AARCH64: u8 = 0x15;
    pub const HVC_AARCH64: u8 = 0x16;
    pub const SMC_AARCH64: u8 = 0x17;
    pub const INSTRUCTION_ABORT_LOWER_EL: u8 = 0x20;
    pub const INSTRUCTION_ABORT_SAME_EL: u8 = 0x21;
    pub const PC_ALIGNMENT_FAULT: u8 = 0x22;
    pub const DATA_ABORT_LOWER_EL: u8 = 0x24;
    pub const DATA_ABORT_SAME_EL: u8 = 0x25;
    pub const SP_ALIGNMENT_FAULT: u8 = 0x26;
    pub const BREAKPOINT_LOWER_EL: u8 = 0x30;
    pub const BREAKPOINT_SAME_EL: u8 = 0x31;
    pub const SOFTWARE_STEP_LOWER_EL: u8 = 0x32;
    pub const SOFTWARE_STEP_SAME_EL: u8 = 0x33;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exception_syndrome_roundtrip() {
        let syndrome = ExceptionSyndrome {
            exception_class: exception_class::SVC_AARCH64,
            instruction_length_32: true,
            iss: 0x1234,
        };
        let esr = syndrome.to_esr();
        let decoded = ExceptionSyndrome::from_esr(esr);
        assert_eq!(decoded.exception_class, syndrome.exception_class);
        assert_eq!(decoded.instruction_length_32, syndrome.instruction_length_32);
        assert_eq!(decoded.iss, syndrome.iss);
    }
}
