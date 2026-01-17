use std::collections::VecDeque;
use std::io::{self, Write};

use super::traits::InterruptSource;
use crate::error::MemoryError;
use crate::memory::BusDevice;

mod register_offset {
    pub const DATA: u64 = 0x00;
    pub const FLAGS: u64 = 0x18;
    pub const INTEGER_BAUD_RATE: u64 = 0x24;
    pub const FRACTIONAL_BAUD_RATE: u64 = 0x28;
    pub const LINE_CONTROL: u64 = 0x2c;
    pub const CONTROL: u64 = 0x30;
    pub const INTERRUPT_MASK: u64 = 0x38;
    pub const RAW_INTERRUPT_STATUS: u64 = 0x3c;
    pub const MASKED_INTERRUPT_STATUS: u64 = 0x40;
    pub const INTERRUPT_CLEAR: u64 = 0x44;
}

mod flag_bits {
    pub const RX_FIFO_EMPTY: u32 = 1 << 4;
}

mod interrupt_bits {
    pub const RECEIVE: u32 = 1 << 4;
    pub const TRANSMIT: u32 = 1 << 5;
}

const FIFO_CAPACITY: usize = 16;
const FRACTIONAL_BAUD_MASK: u32 = 0x3f;

pub trait UartOutput: Send {
    fn write_byte(&mut self, byte: u8);
    fn flush(&mut self);
}

pub struct StdoutOutput;

impl UartOutput for StdoutOutput {
    fn write_byte(&mut self, byte: u8) {
        let _ = io::stdout().write_all(&[byte]);
    }

    fn flush(&mut self) {
        let _ = io::stdout().flush();
    }
}

pub struct NullOutput;

impl UartOutput for NullOutput {
    fn write_byte(&mut self, _byte: u8) {}
    fn flush(&mut self) {}
}

pub struct BufferOutput {
    pub buffer: Vec<u8>,
}

impl BufferOutput {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }
}

impl Default for BufferOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl UartOutput for BufferOutput {
    fn write_byte(&mut self, byte: u8) {
        self.buffer.push(byte);
    }

    fn flush(&mut self) {}
}

pub struct Uart {
    name: String,
    rx_fifo: VecDeque<u8>,
    control: u32,
    line_control: u32,
    integer_baud_rate: u32,
    fractional_baud_rate: u32,
    interrupt_mask: u32,
    raw_interrupt_status: u32,
    output: Box<dyn UartOutput>,
    pub interrupt_pending: bool,
}

impl Uart {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rx_fifo: VecDeque::with_capacity(FIFO_CAPACITY),
            control: 0x0300,
            line_control: 0,
            integer_baud_rate: 0,
            fractional_baud_rate: 0,
            interrupt_mask: 0,
            raw_interrupt_status: 0,
            output: Box::new(StdoutOutput),
            interrupt_pending: false,
        }
    }

    pub fn with_output(name: impl Into<String>, output: Box<dyn UartOutput>) -> Self {
        Self { output, ..Self::new(name) }
    }

    pub fn inject_input(&mut self, byte: u8) {
        if self.rx_fifo.len() < FIFO_CAPACITY {
            self.rx_fifo.push_back(byte);
            self.raw_interrupt_status |= interrupt_bits::RECEIVE;
            self.sync_interrupt_pending();
        }
    }

    pub fn inject_string(&mut self, s: &str) {
        for byte in s.bytes() {
            self.inject_input(byte);
        }
    }

    fn sync_interrupt_pending(&mut self) {
        self.interrupt_pending = (self.raw_interrupt_status & self.interrupt_mask) != 0;
    }

    fn read_flags(&self) -> u32 {
        let mut flags = 0u32;
        if self.rx_fifo.is_empty() {
            flags |= flag_bits::RX_FIFO_EMPTY;
        }
        flags
    }

    fn read_data(&mut self) -> u32 {
        match self.rx_fifo.pop_front() {
            Some(byte) => {
                if self.rx_fifo.is_empty() {
                    self.raw_interrupt_status &= !interrupt_bits::RECEIVE;
                    self.sync_interrupt_pending();
                }
                byte as u32
            }
            None => 0,
        }
    }

    fn write_data(&mut self, value: u32) {
        self.output.write_byte(value as u8);
        self.raw_interrupt_status |= interrupt_bits::TRANSMIT;
        self.sync_interrupt_pending();
    }
}

impl BusDevice for Uart {
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
            DATA => self.read_data(),
            FLAGS => self.read_flags(),
            INTEGER_BAUD_RATE => self.integer_baud_rate,
            FRACTIONAL_BAUD_RATE => self.fractional_baud_rate,
            LINE_CONTROL => self.line_control,
            CONTROL => self.control,
            INTERRUPT_MASK => self.interrupt_mask,
            RAW_INTERRUPT_STATUS => self.raw_interrupt_status,
            MASKED_INTERRUPT_STATUS => self.raw_interrupt_status & self.interrupt_mask,
            _ => 0,
        };
        Ok(value)
    }

    fn write_u32(&mut self, offset: u64, value: u32) -> Result<(), MemoryError> {
        use register_offset::*;

        match offset {
            DATA => self.write_data(value),
            INTEGER_BAUD_RATE => self.integer_baud_rate = value,
            FRACTIONAL_BAUD_RATE => self.fractional_baud_rate = value & FRACTIONAL_BAUD_MASK,
            LINE_CONTROL => self.line_control = value,
            CONTROL => self.control = value,
            INTERRUPT_MASK => {
                self.interrupt_mask = value;
                self.sync_interrupt_pending();
            }
            INTERRUPT_CLEAR => {
                self.raw_interrupt_status &= !value;
                self.sync_interrupt_pending();
            }
            _ => {}
        }
        Ok(())
    }
}

impl InterruptSource for Uart {
    fn has_pending_interrupt(&self) -> bool {
        self.interrupt_pending
    }

    fn interrupt_number(&self) -> Option<u32> {
        if self.interrupt_pending { Some(33) } else { None }
    }

    fn clear_interrupt(&mut self) {
        self.raw_interrupt_status = 0;
        self.sync_interrupt_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use register_offset::*;

    #[test]
    fn transmit_writes_to_output() {
        let output = Box::new(BufferOutput::new());
        let output_ptr = output.as_ref() as *const BufferOutput;
        let mut uart = Uart::with_output("test", output);

        uart.write_u32(DATA, b'H' as u32).unwrap();
        uart.write_u32(DATA, b'i' as u32).unwrap();

        #[allow(unsafe_code)]
        let output = unsafe { &*output_ptr };
        assert_eq!(output.buffer, b"Hi");
    }

    #[test]
    fn receive_reads_from_fifo() {
        let mut uart = Uart::with_output("test", Box::new(NullOutput));
        uart.inject_string("test");

        assert_eq!(uart.read_u32(DATA).unwrap(), b't' as u32);
        assert_eq!(uart.read_u32(DATA).unwrap(), b'e' as u32);
    }

    #[test]
    fn flags_reflect_fifo_state() {
        let mut uart = Uart::with_output("test", Box::new(NullOutput));

        let flags = uart.read_u32(FLAGS).unwrap();
        assert!((flags & flag_bits::RX_FIFO_EMPTY) != 0);

        uart.inject_input(0x42);

        let flags = uart.read_u32(FLAGS).unwrap();
        assert!((flags & flag_bits::RX_FIFO_EMPTY) == 0);
    }
}
