use crate::error::MemoryError;
use crate::memory::BusDevice;

pub struct Ram {
    name: String,
    data: Vec<u8>,
}

impl Ram {
    pub fn new(name: impl Into<String>, size: usize) -> Self {
        Self { name: name.into(), data: vec![0; size] }
    }

    pub fn with_data(name: impl Into<String>, data: Vec<u8>) -> Self {
        Self { name: name.into(), data }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn load(&mut self, offset: usize, data: &[u8]) -> Result<(), MemoryError> {
        if offset + data.len() > self.data.len() {
            return Err(MemoryError::OutOfBounds {
                address: offset as u64,
                size: self.data.len() as u64,
            });
        }
        self.data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    fn check_bounds(&self, offset: u64, access_size: usize) -> Result<(), MemoryError> {
        if offset as usize + access_size > self.data.len() {
            return Err(MemoryError::OutOfBounds { address: offset, size: self.data.len() as u64 });
        }
        Ok(())
    }
}

impl BusDevice for Ram {
    fn name(&self) -> &str {
        &self.name
    }

    fn read_u8(&mut self, offset: u64) -> Result<u8, MemoryError> {
        self.check_bounds(offset, 1)?;
        Ok(self.data[offset as usize])
    }

    fn write_u8(&mut self, offset: u64, value: u8) -> Result<(), MemoryError> {
        self.check_bounds(offset, 1)?;
        self.data[offset as usize] = value;
        Ok(())
    }

    fn read_u16(&mut self, offset: u64) -> Result<u16, MemoryError> {
        self.check_bounds(offset, 2)?;
        let idx = offset as usize;
        Ok(u16::from_le_bytes([self.data[idx], self.data[idx + 1]]))
    }

    fn write_u16(&mut self, offset: u64, value: u16) -> Result<(), MemoryError> {
        self.check_bounds(offset, 2)?;
        let idx = offset as usize;
        let bytes = value.to_le_bytes();
        self.data[idx] = bytes[0];
        self.data[idx + 1] = bytes[1];
        Ok(())
    }

    fn read_u32(&mut self, offset: u64) -> Result<u32, MemoryError> {
        self.check_bounds(offset, 4)?;
        let idx = offset as usize;
        Ok(u32::from_le_bytes([
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]))
    }

    fn write_u32(&mut self, offset: u64, value: u32) -> Result<(), MemoryError> {
        self.check_bounds(offset, 4)?;
        let idx = offset as usize;
        let bytes = value.to_le_bytes();
        self.data[idx..idx + 4].copy_from_slice(&bytes);
        Ok(())
    }

    fn read_u64(&mut self, offset: u64) -> Result<u64, MemoryError> {
        self.check_bounds(offset, 8)?;
        let idx = offset as usize;
        Ok(u64::from_le_bytes([
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
            self.data[idx + 4],
            self.data[idx + 5],
            self.data[idx + 6],
            self.data[idx + 7],
        ]))
    }

    fn write_u64(&mut self, offset: u64, value: u64) -> Result<(), MemoryError> {
        self.check_bounds(offset, 8)?;
        let idx = offset as usize;
        let bytes = value.to_le_bytes();
        self.data[idx..idx + 8].copy_from_slice(&bytes);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ram_basic() {
        let mut ram = Ram::new("test", 1024);
        ram.write_u32(0, 0x12345678).unwrap();
        assert_eq!(ram.read_u32(0).unwrap(), 0x12345678);
    }

    #[test]
    fn test_ram_load() {
        let mut ram = Ram::new("test", 1024);
        ram.load(0, &[1, 2, 3, 4]).unwrap();
        assert_eq!(ram.read_u8(0).unwrap(), 1);
        assert_eq!(ram.read_u8(3).unwrap(), 4);
    }

    #[test]
    fn test_ram_bounds() {
        let mut ram = Ram::new("test", 16);
        assert!(ram.read_u32(16).is_err());
        assert!(ram.read_u32(14).is_err());
    }
}
