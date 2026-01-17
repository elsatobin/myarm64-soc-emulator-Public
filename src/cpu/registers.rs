use crate::error::CpuError;

#[derive(Debug, Clone, Default)]
pub struct Pstate {
    pub n: bool,
    pub z: bool,
    pub c: bool,
    pub v: bool,
    pub el: u8,
    pub sp_sel: bool,
}

impl Pstate {
    pub fn new() -> Self {
        Self { el: 1, ..Default::default() }
    }

    pub fn condition_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.n {
            flags |= 0b1000;
        }
        if self.z {
            flags |= 0b0100;
        }
        if self.c {
            flags |= 0b0010;
        }
        if self.v {
            flags |= 0b0001;
        }
        flags
    }

    pub fn set_condition_flags(&mut self, flags: u8) {
        self.n = (flags & 0b1000) != 0;
        self.z = (flags & 0b0100) != 0;
        self.c = (flags & 0b0010) != 0;
        self.v = (flags & 0b0001) != 0;
    }
}

#[derive(Debug, Clone)]
pub struct Registers {
    pub x: [u64; 31],
    pub sp_el0: u64,
    pub sp_el1: u64,
    pub sp_el2: u64,
    pub sp_el3: u64,
    pub pc: u64,
    pub elr_el1: u64,
    pub spsr_el1: u64,
    pub vbar_el1: u64,
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

impl Registers {
    pub fn new() -> Self {
        Self {
            x: [0; 31],
            sp_el0: 0,
            sp_el1: 0,
            sp_el2: 0,
            sp_el3: 0,
            pc: 0,
            elr_el1: 0,
            spsr_el1: 0,
            vbar_el1: 0,
        }
    }

    pub fn get_x(&self, reg: u8) -> u64 {
        if reg >= 31 {
            return 0;
        }
        self.x[reg as usize]
    }

    pub fn set_x(&mut self, reg: u8, value: u64) {
        if reg < 31 {
            self.x[reg as usize] = value;
        }
    }

    pub fn get_w(&self, reg: u8) -> u32 {
        self.get_x(reg) as u32
    }

    pub fn set_w(&mut self, reg: u8, value: u32) {
        self.set_x(reg, value as u64);
    }
}

#[derive(Debug, Clone)]
pub struct Cpu {
    pub regs: Registers,
    pub pstate: Pstate,
    pub halted: bool,
    pub instruction_count: u64,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self { regs: Registers::new(), pstate: Pstate::new(), halted: false, instruction_count: 0 }
    }

    pub fn reset(&mut self, entry_point: u64) {
        self.regs = Registers::new();
        self.pstate = Pstate::new();
        self.regs.pc = entry_point;
        self.halted = false;
        self.instruction_count = 0;
    }

    pub fn advance_pc(&mut self) {
        self.regs.pc = self.regs.pc.wrapping_add(4);
    }

    pub fn get_sp(&self) -> u64 {
        if !self.pstate.sp_sel {
            return self.regs.sp_el0;
        }
        match self.pstate.el {
            0 => self.regs.sp_el0,
            1 => self.regs.sp_el1,
            2 => self.regs.sp_el2,
            3 => self.regs.sp_el3,
            _ => self.regs.sp_el0,
        }
    }

    pub fn set_sp(&mut self, value: u64) {
        if !self.pstate.sp_sel {
            self.regs.sp_el0 = value;
            return;
        }
        match self.pstate.el {
            0 => self.regs.sp_el0 = value,
            1 => self.regs.sp_el1 = value,
            2 => self.regs.sp_el2 = value,
            3 => self.regs.sp_el3 = value,
            _ => self.regs.sp_el0 = value,
        }
    }

    pub fn get_reg_or_sp(&self, reg: u8, is_sp_context: bool) -> u64 {
        if reg == 31 {
            if is_sp_context {
                return self.get_sp();
            }
            return 0;
        }
        self.regs.get_x(reg)
    }

    pub fn set_reg_or_sp(&mut self, reg: u8, value: u64, is_sp_context: bool) {
        if reg == 31 {
            if is_sp_context {
                self.set_sp(value);
            }
            return;
        }
        self.regs.set_x(reg, value);
    }

    pub fn check_condition(&self, cond: u8) -> Result<bool, CpuError> {
        let n = self.pstate.n;
        let z = self.pstate.z;
        let c = self.pstate.c;
        let v = self.pstate.v;

        let base_result = match cond >> 1 {
            0b000 => z,
            0b001 => c,
            0b010 => n,
            0b011 => v,
            0b100 => c && !z,
            0b101 => n == v,
            0b110 => (n == v) && !z,
            0b111 => true,
            _ => {
                return Err(CpuError::InvalidInstruction {
                    address: self.regs.pc,
                    reason: format!("invalid condition code: {cond:#x}"),
                });
            }
        };

        let should_invert = (cond & 1) != 0 && (cond >> 1) != 0b111;
        Ok(if should_invert { !base_result } else { base_result })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_register_always_reads_zero() {
        let mut regs = Registers::new();
        regs.set_x(31, 0xdeadbeef);
        assert_eq!(regs.get_x(31), 0);
    }

    #[test]
    fn condition_flags_round_trip() {
        let mut pstate = Pstate::new();
        pstate.set_condition_flags(0b1010);
        assert!(pstate.n);
        assert!(!pstate.z);
        assert!(pstate.c);
        assert!(!pstate.v);
        assert_eq!(pstate.condition_flags(), 0b1010);
    }

    #[test]
    fn write_32bit_zero_extends_to_64bit() {
        let mut regs = Registers::new();
        regs.set_x(0, 0xffffffff_ffffffff);
        regs.set_w(0, 0x12345678);
        assert_eq!(regs.get_x(0), 0x12345678);
    }
}
