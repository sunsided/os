use crate::cpuid::{CpuidRanges, CpuidResult, cpuid};

pub const LEAF_15H: u32 = 0x15;

#[derive(Copy, Clone, Debug)]
pub struct Leaf15h {
    pub denom: u32,      // EAX
    pub numer: u32,      // EBX
    pub crystal_hz: u32, // ECX (0 if unknown)
}

impl Leaf15h {
    /// # Safety
    /// The caller must ensure that the `cpuid` instruction is available and leaf `0x16` exists.
    pub unsafe fn new() -> Self {
        unsafe {
            let r = cpuid(LEAF_15H, 0);
            Self::from(r)
        }
    }

    /// Query CPUID.15H if available; None if leaf unsupported.
    pub unsafe fn read(ranges: &CpuidRanges) -> Option<Self> {
        if !ranges.has_basic(LEAF_15H) {
            return None;
        }

        unsafe {
            let r = cpuid(LEAF_15H, 0);
            Some(Self::from(r))
        }
    }

    /// # Safety
    /// The caller must ensure that the passed [`CpuidResult`] belongs to a valid leaf `0x15` entry.
    pub const unsafe fn from(r: CpuidResult) -> Self {
        Self {
            denom: r.eax,
            numer: r.ebx,
            crystal_hz: r.ecx,
        }
    }

    /// Compute TSC frequency if enough info is present.
    pub fn tsc_hz(&self) -> Option<u64> {
        if self.denom != 0 && self.numer != 0 && self.crystal_hz != 0 {
            Some(u64::from(self.crystal_hz) * u64::from(self.numer) / u64::from(self.denom))
        } else {
            None
        }
    }
}
