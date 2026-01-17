use super::traits::{Clocked, InterruptSource};
use crate::error::MemoryError;
use crate::memory::BusDevice;

mod register_offset {
    pub const LOAD: u64 = 0x00;
    pub const VALUE: u64 = 0x04;
    pub const CONTROL: u64 = 0x08;
    pub const INTERRUPT_CLEAR: u64 = 0x0c;
    pub const RAW_INTERRUPT_STATUS: u64 = 0x10;
    pub const MASKED_INTERRUPT_STATUS: u64 = 0x14;
    pub const BACKGROUND_LOAD: u64 = 0x18;
}

mod control_flags {
    pub const ENABLED: u32 = 1 << 7;
    pub const PERIODIC_MODE: u32 = 1 << 6;
    pub const INTERRUPT_ENABLED: u32 = 1 << 5;
    pub const PRESCALE_BITS: u32 = 0x0c;
    pub const ONESHOT_MODE: u32 = 1 << 0;
}

const MAX_COUNTER_VALUE: u32 = 0xffffffff;

pub struct Timer {
    name: String,
    load_value: u32,
    counter: u32,
    background_load: u32,
    control: u32,
    interrupt_fired: bool,
    prescale_counter: u32,
    pub interrupt_pending: bool,
}

impl Timer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            load_value: 0,
            counter: MAX_COUNTER_VALUE,
            background_load: 0,
            control: 0,
            interrupt_fired: false,
            prescale_counter: 0,
            interrupt_pending: false,
        }
    }

    pub fn tick(&mut self) {
        if !self.is_enabled() {
            return;
        }

        if !self.prescale_tick() {
            return;
        }

        if self.counter == 0 {
            self.handle_underflow();
        } else {
            self.counter -= 1;
        }
    }

    fn prescale_tick(&mut self) -> bool {
        let prescale_divisor = self.prescale_divisor();
        self.prescale_counter += 1;

        if self.prescale_counter < prescale_divisor {
            return false;
        }

        self.prescale_counter = 0;
        true
    }

    fn prescale_divisor(&self) -> u32 {
        match (self.control & control_flags::PRESCALE_BITS) >> 2 {
            1 => 16,
            2 => 256,
            _ => 1,
        }
    }

    fn handle_underflow(&mut self) {
        self.interrupt_fired = true;
        self.sync_interrupt_pending();

        if self.is_periodic() {
            self.counter = self.load_value;
        } else if self.is_oneshot() {
            self.control &= !control_flags::ENABLED;
        } else {
            self.counter = MAX_COUNTER_VALUE;
        }
    }

    fn sync_interrupt_pending(&mut self) {
        self.interrupt_pending = self.interrupt_fired && self.is_interrupt_enabled();
    }

    fn is_enabled(&self) -> bool {
        (self.control & control_flags::ENABLED) != 0
    }

    fn is_periodic(&self) -> bool {
        (self.control & control_flags::PERIODIC_MODE) != 0
    }

    fn is_oneshot(&self) -> bool {
        (self.control & control_flags::ONESHOT_MODE) != 0
    }

    fn is_interrupt_enabled(&self) -> bool {
        (self.control & control_flags::INTERRUPT_ENABLED) != 0
    }

    pub fn advance(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    pub fn get_value(&self) -> u32 {
        self.counter
    }
}

impl BusDevice for Timer {
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
        use register_offset::*;

        let value = match offset {
            LOAD => self.load_value,
            VALUE => self.counter,
            CONTROL => self.control,
            RAW_INTERRUPT_STATUS => u32::from(self.interrupt_fired),
            MASKED_INTERRUPT_STATUS => u32::from(self.interrupt_pending),
            BACKGROUND_LOAD => self.background_load,
            _ => 0,
        };
        Ok(value)
    }

    fn write_u32(&mut self, offset: u64, value: u32) -> Result<(), MemoryError> {
        use register_offset::*;

        match offset {
            LOAD => {
                self.load_value = value;
                self.counter = value;
            }
            CONTROL => {
                self.control = value;
                self.sync_interrupt_pending();
            }
            INTERRUPT_CLEAR => {
                self.interrupt_fired = false;
                self.sync_interrupt_pending();
            }
            BACKGROUND_LOAD => {
                self.background_load = value;
                self.load_value = value;
            }
            _ => {}
        }
        Ok(())
    }
}

impl InterruptSource for Timer {
    fn has_pending_interrupt(&self) -> bool {
        self.interrupt_pending
    }

    fn interrupt_number(&self) -> Option<u32> {
        if self.interrupt_pending { Some(30) } else { None }
    }

    fn clear_interrupt(&mut self) {
        self.interrupt_fired = false;
        self.sync_interrupt_pending();
    }
}

impl Clocked for Timer {
    fn tick(&mut self, cycles: u64) {
        for _ in 0..cycles {
            Self::tick(self);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use control_flags::*;
    use register_offset::*;

    #[test]
    fn countdown_decrements_each_tick() {
        let mut timer = Timer::new("test");
        timer.write_u32(LOAD, 10).unwrap();
        timer.write_u32(CONTROL, ENABLED | PERIODIC_MODE).unwrap();

        for _ in 0..5 {
            timer.tick();
        }

        assert_eq!(timer.get_value(), 5);
    }

    #[test]
    fn periodic_mode_reloads_on_underflow() {
        let mut timer = Timer::new("test");
        timer.write_u32(LOAD, 2).unwrap();
        timer.write_u32(CONTROL, ENABLED | PERIODIC_MODE | INTERRUPT_ENABLED).unwrap();

        timer.advance(3);

        assert!(timer.interrupt_pending);
        assert_eq!(timer.get_value(), 2);
    }

    #[test]
    fn disabled_timer_does_not_count() {
        let mut timer = Timer::new("test");
        timer.write_u32(LOAD, 10).unwrap();

        timer.advance(100);

        assert_eq!(timer.get_value(), 10);
    }
}
