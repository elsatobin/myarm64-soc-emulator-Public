use crate::error::{BusError, MemoryError};

pub trait BusDevice: Send {
    fn name(&self) -> &str;
    fn read_u8(&mut self, offset: u64) -> Result<u8, MemoryError>;
    fn write_u8(&mut self, offset: u64, value: u8) -> Result<(), MemoryError>;

    fn read_u16(&mut self, offset: u64) -> Result<u16, MemoryError> {
        let lo = self.read_u8(offset)?;
        let hi = self.read_u8(offset + 1)?;
        Ok(u16::from_le_bytes([lo, hi]))
    }

    fn write_u16(&mut self, offset: u64, value: u16) -> Result<(), MemoryError> {
        let bytes = value.to_le_bytes();
        self.write_u8(offset, bytes[0])?;
        self.write_u8(offset + 1, bytes[1])?;
        Ok(())
    }

    fn read_u32(&mut self, offset: u64) -> Result<u32, MemoryError> {
        let b0 = self.read_u8(offset)?;
        let b1 = self.read_u8(offset + 1)?;
        let b2 = self.read_u8(offset + 2)?;
        let b3 = self.read_u8(offset + 3)?;
        Ok(u32::from_le_bytes([b0, b1, b2, b3]))
    }

    fn write_u32(&mut self, offset: u64, value: u32) -> Result<(), MemoryError> {
        let bytes = value.to_le_bytes();
        self.write_u8(offset, bytes[0])?;
        self.write_u8(offset + 1, bytes[1])?;
        self.write_u8(offset + 2, bytes[2])?;
        self.write_u8(offset + 3, bytes[3])?;
        Ok(())
    }

    fn read_u64(&mut self, offset: u64) -> Result<u64, MemoryError> {
        let lo = self.read_u32(offset)?;
        let hi = self.read_u32(offset + 4)?;
        Ok(((hi as u64) << 32) | (lo as u64))
    }

    fn write_u64(&mut self, offset: u64, value: u64) -> Result<(), MemoryError> {
        self.write_u32(offset, value as u32)?;
        self.write_u32(offset + 4, (value >> 32) as u32)?;
        Ok(())
    }
}

pub trait Bus {
    fn read_u8(&mut self, address: u64) -> Result<u8, MemoryError>;
    fn read_u16(&mut self, address: u64) -> Result<u16, MemoryError>;
    fn read_u32(&mut self, address: u64) -> Result<u32, MemoryError>;
    fn read_u64(&mut self, address: u64) -> Result<u64, MemoryError>;

    fn write_u8(&mut self, address: u64, value: u8) -> Result<(), MemoryError>;
    fn write_u16(&mut self, address: u64, value: u16) -> Result<(), MemoryError>;
    fn write_u32(&mut self, address: u64, value: u32) -> Result<(), MemoryError>;
    fn write_u64(&mut self, address: u64, value: u64) -> Result<(), MemoryError>;

    fn read_bytes(&mut self, address: u64, buffer: &mut [u8]) -> Result<(), MemoryError> {
        for (i, byte) in buffer.iter_mut().enumerate() {
            *byte = self.read_u8(address + i as u64)?;
        }
        Ok(())
    }

    fn write_bytes(&mut self, address: u64, data: &[u8]) -> Result<(), MemoryError> {
        for (i, byte) in data.iter().enumerate() {
            self.write_u8(address + i as u64, *byte)?;
        }
        Ok(())
    }
}

pub struct MemoryRegion {
    pub start: u64,
    pub size: u64,
    pub device: Box<dyn BusDevice>,
}

impl MemoryRegion {
    pub fn new(start: u64, size: u64, device: Box<dyn BusDevice>) -> Self {
        Self { start, size, device }
    }

    pub fn contains(&self, address: u64) -> bool {
        address >= self.start && address < self.start + self.size
    }

    pub fn offset(&self, address: u64) -> u64 {
        address - self.start
    }
}

pub struct SimpleBus {
    regions: Vec<MemoryRegion>,
}

impl SimpleBus {
    pub fn new() -> Self {
        Self { regions: Vec::new() }
    }

    pub fn add_region(&mut self, region: MemoryRegion) -> Result<(), BusError> {
        for existing in &self.regions {
            let new_end = region.start + region.size;
            let existing_end = existing.start + existing.size;

            let overlaps = region.start < existing_end && new_end > existing.start;
            if overlaps {
                return Err(BusError::OverlappingRange { start: region.start, end: new_end });
            }
        }

        self.regions.push(region);
        Ok(())
    }

    fn find_region(&mut self, address: u64) -> Result<&mut MemoryRegion, MemoryError> {
        for region in &mut self.regions {
            if region.contains(address) {
                return Ok(region);
            }
        }
        Err(MemoryError::OutOfBounds { address, size: 0 })
    }
}

impl Default for SimpleBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for SimpleBus {
    fn read_u8(&mut self, address: u64) -> Result<u8, MemoryError> {
        let region = self.find_region(address)?;
        let offset = region.offset(address);
        region.device.read_u8(offset)
    }

    fn read_u16(&mut self, address: u64) -> Result<u16, MemoryError> {
        let region = self.find_region(address)?;
        let offset = region.offset(address);
        region.device.read_u16(offset)
    }

    fn read_u32(&mut self, address: u64) -> Result<u32, MemoryError> {
        let region = self.find_region(address)?;
        let offset = region.offset(address);
        region.device.read_u32(offset)
    }

    fn read_u64(&mut self, address: u64) -> Result<u64, MemoryError> {
        let region = self.find_region(address)?;
        let offset = region.offset(address);
        region.device.read_u64(offset)
    }

    fn write_u8(&mut self, address: u64, value: u8) -> Result<(), MemoryError> {
        let region = self.find_region(address)?;
        let offset = region.offset(address);
        region.device.write_u8(offset, value)
    }

    fn write_u16(&mut self, address: u64, value: u16) -> Result<(), MemoryError> {
        let region = self.find_region(address)?;
        let offset = region.offset(address);
        region.device.write_u16(offset, value)
    }

    fn write_u32(&mut self, address: u64, value: u32) -> Result<(), MemoryError> {
        let region = self.find_region(address)?;
        let offset = region.offset(address);
        region.device.write_u32(offset, value)
    }

    fn write_u64(&mut self, address: u64, value: u64) -> Result<(), MemoryError> {
        let region = self.find_region(address)?;
        let offset = region.offset(address);
        region.device.write_u64(offset, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyDevice {
        data: [u8; 256],
    }

    impl DummyDevice {
        fn new() -> Self {
            Self { data: [0; 256] }
        }
    }

    impl BusDevice for DummyDevice {
        fn name(&self) -> &str {
            "dummy"
        }

        fn read_u8(&mut self, offset: u64) -> Result<u8, MemoryError> {
            if offset >= 256 {
                return Err(MemoryError::OutOfBounds { address: offset, size: 256 });
            }
            Ok(self.data[offset as usize])
        }

        fn write_u8(&mut self, offset: u64, value: u8) -> Result<(), MemoryError> {
            if offset >= 256 {
                return Err(MemoryError::OutOfBounds { address: offset, size: 256 });
            }
            self.data[offset as usize] = value;
            Ok(())
        }
    }

    #[test]
    fn test_bus_read_write() {
        let mut bus = SimpleBus::new();
        let device = Box::new(DummyDevice::new());
        bus.add_region(MemoryRegion::new(0x1000, 256, device)).unwrap();

        bus.write_u8(0x1000, 0x42).unwrap();
        assert_eq!(bus.read_u8(0x1000).unwrap(), 0x42);

        bus.write_u32(0x1010, 0xdeadbeef).unwrap();
        assert_eq!(bus.read_u32(0x1010).unwrap(), 0xdeadbeef);
    }

    #[test]
    fn test_unmapped_address() {
        let mut bus = SimpleBus::new();
        let result = bus.read_u8(0x5000);
        assert!(result.is_err());
    }
}
