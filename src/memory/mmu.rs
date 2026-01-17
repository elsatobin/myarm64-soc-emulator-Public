use crate::error::MemoryError;

#[derive(Debug, Clone, Copy, Default)]
pub struct PageAttributes {
    pub valid: bool,
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub user: bool,
    pub accessed: bool,
    pub dirty: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TlbEntry {
    pub vpn: u64,
    pub ppn: u64,
    pub asid: u16,
    pub attrs: PageAttributes,
    pub valid: bool,
}

#[derive(Debug)]
pub struct Translation {
    pub physical_address: u64,
    pub attrs: PageAttributes,
}

#[derive(Debug, Clone)]
pub struct MmuConfig {
    pub page_size: u64,
    pub tlb_size: usize,
    pub enabled: bool,
}

impl Default for MmuConfig {
    fn default() -> Self {
        Self { page_size: 4096, tlb_size: 64, enabled: false }
    }
}

pub struct Mmu {
    config: MmuConfig,
    tlb: Vec<TlbEntry>,
    pub ttbr0: u64,
    pub ttbr1: u64,
    pub tcr: u64,
    pub asid: u16,
}

impl Mmu {
    pub fn new(config: MmuConfig) -> Self {
        let tlb_size = config.tlb_size;
        Self {
            config,
            tlb: vec![TlbEntry::default(); tlb_size],
            ttbr0: 0,
            ttbr1: 0,
            tcr: 0,
            asid: 0,
        }
    }

    pub fn translate(
        &mut self,
        virtual_address: u64,
        write: bool,
    ) -> Result<Translation, MemoryError> {
        if !self.config.enabled {
            return Ok(Translation {
                physical_address: virtual_address,
                attrs: PageAttributes {
                    valid: true,
                    read: true,
                    write: true,
                    execute: true,
                    user: true,
                    accessed: true,
                    dirty: true,
                },
            });
        }

        let page_mask = self.config.page_size - 1;
        let vpn = virtual_address & !page_mask;
        let offset = virtual_address & page_mask;

        if let Some(entry) = self.tlb_lookup(vpn) {
            if write && !entry.attrs.write {
                return Err(MemoryError::PermissionDenied("write to read-only page".to_string()));
            }

            return Ok(Translation { physical_address: entry.ppn | offset, attrs: entry.attrs });
        }

        Err(MemoryError::PageFault { address: virtual_address })
    }

    fn tlb_lookup(&self, vpn: u64) -> Option<&TlbEntry> {
        self.tlb.iter().find(|entry| entry.valid && entry.vpn == vpn && entry.asid == self.asid)
    }

    pub fn tlb_insert(&mut self, vpn: u64, ppn: u64, attrs: PageAttributes) {
        let mut insert_idx = 0;
        for (i, entry) in self.tlb.iter().enumerate() {
            if !entry.valid {
                insert_idx = i;
                break;
            }
            insert_idx = (insert_idx + 1) % self.tlb.len();
        }

        self.tlb[insert_idx] = TlbEntry { vpn, ppn, asid: self.asid, attrs, valid: true };
    }

    pub fn tlb_flush_all(&mut self) {
        for entry in &mut self.tlb {
            entry.valid = false;
        }
    }

    pub fn tlb_flush_asid(&mut self, asid: u16) {
        for entry in &mut self.tlb {
            if entry.asid == asid {
                entry.valid = false;
            }
        }
    }

    pub fn tlb_flush_va(&mut self, virtual_address: u64) {
        let page_mask = self.config.page_size - 1;
        let vpn = virtual_address & !page_mask;

        for entry in &mut self.tlb {
            if entry.vpn == vpn && entry.asid == self.asid {
                entry.valid = false;
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
        if !enabled {
            self.tlb_flush_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mmu_disabled_identity() {
        let mut mmu = Mmu::new(MmuConfig::default());
        let result = mmu.translate(0x12345678, false).unwrap();
        assert_eq!(result.physical_address, 0x12345678);
    }

    #[test]
    fn test_tlb_insert_lookup() {
        let mut mmu = Mmu::new(MmuConfig { enabled: true, ..Default::default() });

        let attrs = PageAttributes {
            valid: true,
            read: true,
            write: true,
            execute: false,
            user: true,
            accessed: true,
            dirty: false,
        };

        mmu.tlb_insert(0x1000, 0x2000, attrs);

        let result = mmu.translate(0x1234, false).unwrap();
        assert_eq!(result.physical_address, 0x2234);
    }
}
