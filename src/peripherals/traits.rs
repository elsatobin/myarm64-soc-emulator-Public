use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::error::MemoryError;
use crate::memory::BusDevice;

pub trait InterruptSource {
    fn has_pending_interrupt(&self) -> bool;
    fn interrupt_number(&self) -> Option<u32>;
    fn clear_interrupt(&mut self);
}

pub trait Clocked {
    fn tick(&mut self, cycles: u64);
}

pub trait InterruptingDevice: BusDevice + InterruptSource {}

impl<T: BusDevice + InterruptSource> InterruptingDevice for T {}

pub struct SharedPeripheral<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> SharedPeripheral<T> {
    pub fn new(peripheral: T) -> Self {
        Self { inner: Arc::new(Mutex::new(peripheral)) }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn bus_adapter(&self) -> SharedBusAdapter<T> {
        SharedBusAdapter { inner: Arc::clone(&self.inner) }
    }
}

impl<T: Clone> Clone for SharedPeripheral<T> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<T: InterruptSource> InterruptSource for SharedPeripheral<T> {
    fn has_pending_interrupt(&self) -> bool {
        self.lock().has_pending_interrupt()
    }

    fn interrupt_number(&self) -> Option<u32> {
        self.lock().interrupt_number()
    }

    fn clear_interrupt(&mut self) {
        self.lock().clear_interrupt();
    }
}

impl<T: Clocked> Clocked for SharedPeripheral<T> {
    fn tick(&mut self, cycles: u64) {
        self.lock().tick(cycles);
    }
}

pub struct SharedBusAdapter<T> {
    inner: Arc<Mutex<T>>,
}

impl<T: BusDevice> BusDevice for SharedBusAdapter<T> {
    fn name(&self) -> &str {
        "shared-adapter"
    }

    fn read_u8(&mut self, offset: u64) -> Result<u8, MemoryError> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).read_u8(offset)
    }

    fn write_u8(&mut self, offset: u64, value: u8) -> Result<(), MemoryError> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).write_u8(offset, value)
    }

    fn read_u32(&mut self, offset: u64) -> Result<u32, MemoryError> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).read_u32(offset)
    }

    fn write_u32(&mut self, offset: u64, value: u32) -> Result<(), MemoryError> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner).write_u32(offset, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peripherals::Timer;

    #[test]
    fn shared_peripheral_allows_concurrent_access_patterns() {
        let shared_timer = SharedPeripheral::new(Timer::new("shared-timer"));

        {
            let mut timer = shared_timer.lock();
            timer.write_u32(0x00, 100).unwrap();
            timer.write_u32(0x08, 0xe0).unwrap();
        }

        assert!(!shared_timer.has_pending_interrupt());

        let bus_adapter = shared_timer.bus_adapter();
        let mut adapter = bus_adapter;
        assert_eq!(adapter.read_u32(0x04).unwrap(), 100);
    }
}
