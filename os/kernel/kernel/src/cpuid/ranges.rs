use crate::cpuid::cpuid;

const LEAF_00H: u32 = 0b00;
const LEAF_MAX_EXTENDED: u32 = 0x8000_0000;

#[derive(Copy, Clone)]
pub struct CpuidRanges {
    pub max_basic: u32,
    pub max_extended: u32,
    pub vendor: CpuVendor,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CpuVendor {
    Intel,
    Amd,
    Other,
}

impl CpuidRanges {
    pub unsafe fn read() -> Self {
        let b0 = unsafe { cpuid(LEAF_00H, 0) };
        let max_basic = b0.eax;

        let v = [b0.ebx, b0.edx, b0.ecx]; // e.g., "GenuineIntel", "AuthenticAMD"
        let ptr: &[u8] = unsafe { core::slice::from_raw_parts(v.as_ptr() as *const u8, 12) };

        let vendor = match unsafe { core::str::from_utf8_unchecked(ptr).trim_end_matches('\0') } {
            "GenuineIntel" => CpuVendor::Intel,
            "AuthenticAMD" => CpuVendor::Amd,
            _ => CpuVendor::Other,
        };

        let e0 = unsafe { cpuid(LEAF_MAX_EXTENDED, 0) };
        let max_extended = e0.eax;

        Self {
            max_basic,
            max_extended,
            vendor,
        }
    }

    #[inline]
    pub fn has_basic(&self, leaf: u32) -> bool {
        leaf <= self.max_basic
    }

    #[inline]
    pub fn has_ext(&self, leaf: u32) -> bool {
        leaf >= 0x8000_0000 && leaf <= self.max_extended
    }
}

impl CpuVendor {
    pub const fn as_str(&self) -> &'static str {
        match self {
            CpuVendor::Intel => "Intel",
            CpuVendor::Amd => "AMD",
            CpuVendor::Other => "Other",
        }
    }
}
