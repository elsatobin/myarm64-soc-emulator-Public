use crate::error::MemoryError;
use crate::memory::BusDevice;

pub const MAX_INTERRUPTS: usize = 256;
const SPURIOUS_INTERRUPT: u32 = 1023;
const INTERRUPTS_PER_WORD: usize = 32;
const BITMAP_WORDS: usize = MAX_INTERRUPTS / INTERRUPTS_PER_WORD;

mod distributor_register {
    pub const CONTROL: u64 = 0x000;
    pub const TYPE: u64 = 0x004;
    pub const SET_ENABLE_BASE: u64 = 0x100;
    pub const CLEAR_ENABLE_BASE: u64 = 0x180;
    pub const SET_PENDING_BASE: u64 = 0x200;
    pub const CLEAR_PENDING_BASE: u64 = 0x280;
    pub const PRIORITY_BASE: u64 = 0x400;
    pub const TARGET_BASE: u64 = 0x800;
}

mod cpu_interface_register {
    pub const CONTROL: u64 = 0x00;
    pub const PRIORITY_MASK: u64 = 0x04;
    pub const ACKNOWLEDGE: u64 = 0x0c;
    pub const END_OF_INTERRUPT: u64 = 0x10;
    pub const RUNNING_PRIORITY: u64 = 0x14;
    pub const HIGHEST_PENDING: u64 = 0x18;
}

fn interrupt_bitmap_index(irq: u32) -> (usize, u32) {
    let word_index = (irq / INTERRUPTS_PER_WORD as u32) as usize;
    let bit_position = irq % INTERRUPTS_PER_WORD as u32;
    (word_index, bit_position)
}

pub struct GicDistributor {
    name: String,
    enabled: bool,
    enable_bitmap: [u32; BITMAP_WORDS],
    pending_bitmap: [u32; BITMAP_WORDS],
    priority: [u8; MAX_INTERRUPTS],
    cpu_targets: [u8; MAX_INTERRUPTS],
}

impl GicDistributor {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: false,
            enable_bitmap: [0; BITMAP_WORDS],
            pending_bitmap: [0; BITMAP_WORDS],
            priority: [0; MAX_INTERRUPTS],
            cpu_targets: [0; MAX_INTERRUPTS],
        }
    }

    pub fn set_pending(&mut self, irq: u32) {
        if irq < MAX_INTERRUPTS as u32 {
            let (word, bit) = interrupt_bitmap_index(irq);
            self.pending_bitmap[word] |= 1 << bit;
        }
    }

    pub fn clear_pending(&mut self, irq: u32) {
        if irq < MAX_INTERRUPTS as u32 {
            let (word, bit) = interrupt_bitmap_index(irq);
            self.pending_bitmap[word] &= !(1 << bit);
        }
    }

    fn is_interrupt_enabled(&self, irq: u32) -> bool {
        if irq >= MAX_INTERRUPTS as u32 {
            return false;
        }
        let (word, bit) = interrupt_bitmap_index(irq);
        (self.enable_bitmap[word] & (1 << bit)) != 0
    }

    fn is_interrupt_pending(&self, irq: u32) -> bool {
        if irq >= MAX_INTERRUPTS as u32 {
            return false;
        }
        let (word, bit) = interrupt_bitmap_index(irq);
        (self.pending_bitmap[word] & (1 << bit)) != 0
    }

    pub fn find_highest_priority_pending(
        &self,
        cpu_mask: u8,
        priority_threshold: u8,
    ) -> Option<u32> {
        if !self.enabled {
            return None;
        }

        let mut best_irq = None;
        let mut best_priority = u8::MAX;

        for irq in 0..MAX_INTERRUPTS as u32 {
            if !self.is_interrupt_enabled(irq) {
                continue;
            }
            if !self.is_interrupt_pending(irq) {
                continue;
            }

            let targets_this_cpu = (self.cpu_targets[irq as usize] & cpu_mask) != 0;
            if !targets_this_cpu {
                continue;
            }

            let irq_priority = self.priority[irq as usize];
            let below_threshold = irq_priority < priority_threshold;
            let higher_priority_than_best = irq_priority < best_priority;

            if below_threshold && higher_priority_than_best {
                best_priority = irq_priority;
                best_irq = Some(irq);
            }
        }

        best_irq
    }

    fn read_bitmap_register(&self, base: u64, offset: u64, bitmap: &[u32; BITMAP_WORDS]) -> u32 {
        let index = ((offset - base) / 4) as usize;
        if index < BITMAP_WORDS { bitmap[index] } else { 0 }
    }

    fn set_enable_bits(&mut self, offset: u64, value: u32) {
        let index = ((offset - distributor_register::SET_ENABLE_BASE) / 4) as usize;
        if index < BITMAP_WORDS {
            self.enable_bitmap[index] |= value;
        }
    }

    fn clear_enable_bits(&mut self, offset: u64, value: u32) {
        let index = ((offset - distributor_register::CLEAR_ENABLE_BASE) / 4) as usize;
        if index < BITMAP_WORDS {
            self.enable_bitmap[index] &= !value;
        }
    }

    fn set_pending_bits(&mut self, offset: u64, value: u32) {
        let index = ((offset - distributor_register::SET_PENDING_BASE) / 4) as usize;
        if index < BITMAP_WORDS {
            self.pending_bitmap[index] |= value;
        }
    }

    fn clear_pending_bits(&mut self, offset: u64, value: u32) {
        let index = ((offset - distributor_register::CLEAR_PENDING_BASE) / 4) as usize;
        if index < BITMAP_WORDS {
            self.pending_bitmap[index] &= !value;
        }
    }
}

impl BusDevice for GicDistributor {
    fn name(&self) -> &str {
        &self.name
    }

    fn read_u8(&mut self, offset: u64) -> Result<u8, MemoryError> {
        use distributor_register::*;

        let priority_range = PRIORITY_BASE..PRIORITY_BASE + MAX_INTERRUPTS as u64;
        let target_range = TARGET_BASE..TARGET_BASE + MAX_INTERRUPTS as u64;

        if priority_range.contains(&offset) {
            let irq = (offset - PRIORITY_BASE) as usize;
            return Ok(self.priority[irq]);
        }
        if target_range.contains(&offset) {
            let irq = (offset - TARGET_BASE) as usize;
            return Ok(self.cpu_targets[irq]);
        }
        self.read_u32(offset & !3).map(|v| (v >> ((offset & 3) * 8)) as u8)
    }

    fn write_u8(&mut self, offset: u64, value: u8) -> Result<(), MemoryError> {
        use distributor_register::*;

        let priority_range = PRIORITY_BASE..PRIORITY_BASE + MAX_INTERRUPTS as u64;
        let target_range = TARGET_BASE..TARGET_BASE + MAX_INTERRUPTS as u64;

        if priority_range.contains(&offset) {
            let irq = (offset - PRIORITY_BASE) as usize;
            self.priority[irq] = value;
            return Ok(());
        }
        if target_range.contains(&offset) {
            let irq = (offset - TARGET_BASE) as usize;
            self.cpu_targets[irq] = value;
            return Ok(());
        }
        Ok(())
    }

    fn read_u32(&mut self, offset: u64) -> Result<u32, MemoryError> {
        use distributor_register::*;

        let value = match offset {
            CONTROL => u32::from(self.enabled),
            TYPE => ((MAX_INTERRUPTS / 32 - 1) as u32) & 0x1f,
            o if (SET_ENABLE_BASE..SET_ENABLE_BASE + 32).contains(&o) => {
                self.read_bitmap_register(SET_ENABLE_BASE, o, &self.enable_bitmap)
            }
            o if (SET_PENDING_BASE..SET_PENDING_BASE + 32).contains(&o) => {
                self.read_bitmap_register(SET_PENDING_BASE, o, &self.pending_bitmap)
            }
            _ => 0,
        };
        Ok(value)
    }

    fn write_u32(&mut self, offset: u64, value: u32) -> Result<(), MemoryError> {
        use distributor_register::*;

        match offset {
            CONTROL => self.enabled = (value & 1) != 0,
            o if (SET_ENABLE_BASE..SET_ENABLE_BASE + 32).contains(&o) => {
                self.set_enable_bits(o, value);
            }
            o if (CLEAR_ENABLE_BASE..CLEAR_ENABLE_BASE + 32).contains(&o) => {
                self.clear_enable_bits(o, value);
            }
            o if (SET_PENDING_BASE..SET_PENDING_BASE + 32).contains(&o) => {
                self.set_pending_bits(o, value);
            }
            o if (CLEAR_PENDING_BASE..CLEAR_PENDING_BASE + 32).contains(&o) => {
                self.clear_pending_bits(o, value);
            }
            _ => {}
        }
        Ok(())
    }
}

pub struct GicCpuInterface {
    name: String,
    enabled: bool,
    priority_mask: u8,
    running_priority: u8,
    active_interrupt: Option<u32>,
}

impl GicCpuInterface {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            enabled: false,
            priority_mask: u8::MAX,
            running_priority: u8::MAX,
            active_interrupt: None,
        }
    }

    pub fn acknowledge(&mut self, irq: u32, priority: u8) {
        self.active_interrupt = Some(irq);
        self.running_priority = priority;
    }

    pub fn end_of_interrupt(&mut self) {
        self.active_interrupt = None;
        self.running_priority = u8::MAX;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn get_priority_mask(&self) -> u8 {
        self.priority_mask
    }
}

impl BusDevice for GicCpuInterface {
    fn name(&self) -> &str {
        &self.name
    }

    fn read_u8(&mut self, offset: u64) -> Result<u8, MemoryError> {
        self.read_u32(offset).map(|v| v as u8)
    }

    fn write_u8(&mut self, offset: u64, value: u8) -> Result<(), MemoryError> {
        self.write_u32(offset, value as u32)
    }

    fn read_u32(&mut self, offset: u64) -> Result<u32, MemoryError> {
        use cpu_interface_register::*;

        let value = match offset {
            CONTROL => u32::from(self.enabled),
            PRIORITY_MASK => self.priority_mask as u32,
            ACKNOWLEDGE | HIGHEST_PENDING => self.active_interrupt.unwrap_or(SPURIOUS_INTERRUPT),
            RUNNING_PRIORITY => self.running_priority as u32,
            _ => 0,
        };
        Ok(value)
    }

    fn write_u32(&mut self, offset: u64, value: u32) -> Result<(), MemoryError> {
        use cpu_interface_register::*;

        match offset {
            CONTROL => self.enabled = (value & 1) != 0,
            PRIORITY_MASK => self.priority_mask = value as u8,
            END_OF_INTERRUPT => self.end_of_interrupt(),
            _ => {}
        }
        Ok(())
    }
}

pub struct Gic {
    pub distributor: GicDistributor,
    pub cpu_interface: GicCpuInterface,
}

impl Gic {
    pub fn new() -> Self {
        Self {
            distributor: GicDistributor::new("gic-dist"),
            cpu_interface: GicCpuInterface::new("gic-cpu"),
        }
    }

    pub fn has_pending_interrupt(&self) -> bool {
        if !self.cpu_interface.is_enabled() {
            return false;
        }
        self.distributor
            .find_highest_priority_pending(1, self.cpu_interface.get_priority_mask())
            .is_some()
    }

    pub fn acknowledge_interrupt(&mut self) -> Option<u32> {
        if !self.cpu_interface.is_enabled() {
            return None;
        }

        let irq = self
            .distributor
            .find_highest_priority_pending(1, self.cpu_interface.get_priority_mask())?;
        let priority = self.distributor.priority[irq as usize];
        self.cpu_interface.acknowledge(irq, priority);
        self.distributor.clear_pending(irq);
        Some(irq)
    }
}

impl Default for Gic {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acknowledges_highest_priority_pending_interrupt() {
        let mut gic = Gic::new();

        gic.distributor.enabled = true;
        gic.cpu_interface.enabled = true;

        gic.distributor.enable_bitmap[1] = 1;
        gic.distributor.cpu_targets[32] = 1;
        gic.distributor.priority[32] = 0x80;

        gic.distributor.set_pending(32);

        assert!(gic.has_pending_interrupt());

        let irq = gic.acknowledge_interrupt();
        assert_eq!(irq, Some(32));
        assert!(!gic.has_pending_interrupt());
    }
}
