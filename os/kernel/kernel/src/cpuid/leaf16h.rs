use crate::cpuid::{CpuidRanges, CpuidResult, cpuid};

pub const LEAF_16H: u32 = 0x16;

/// CPUID.16H — Processor Frequency Information (Intel, advisory).
///
/// Returns nominal/base, maximum (single-core turbo), and bus/reference
/// frequencies in **MHz**. Values are informational and may be zero or
/// unimplemented on some CPUs/hypervisors.
///
/// - EAX = Base (nominal) frequency in MHz
/// - EBX = Maximum frequency in MHz
/// - ECX = Bus/Reference frequency in MHz
/// - EDX = Reserved
///
/// Use 16H only if the leaf is reported present by CPUID.(0).EAX.
/// Prefer CPUID.15H for **TSC** frequency when available; 16H is useful
/// as a fallback or for UI/telemetry.
#[derive(Copy, Clone, Debug)]
pub struct Leaf16 {
    /// Nominal/base frequency in MHz (EAX). 0 means “not reported”.
    pub base_mhz: u32,
    /// Maximum frequency in MHz (EBX). 0 means “not reported”.
    pub max_mhz: u32,
    /// Bus/reference clock in MHz (ECX). 0 means “not reported”.
    pub bus_mhz: u32,
}

impl Leaf16 {
    /// # Safety
    /// The caller must ensure that the `cpuid` instruction is available and leaf `0x16` exists.
    pub unsafe fn new() -> Self {
        unsafe {
            let r = cpuid(LEAF_16H, 0);
            Self::from(r)
        }
    }

    /// Query CPUID.16H if available; None if the leaf is unsupported.
    #[inline]
    pub unsafe fn read(ranges: &CpuidRanges) -> Option<Self> {
        if !ranges.has_basic(LEAF_16H) {
            return None;
        }

        unsafe {
            let r = cpuid(LEAF_16H, 0);
            Some(Self::from(r))
        }
    }

    /// # Safety
    /// The caller must ensure that the passed [`CpuidResult`] belongs to a valid leaf `0x16` entry.
    pub const unsafe fn from(r: CpuidResult) -> Self {
        Self {
            base_mhz: r.eax,
            max_mhz: r.ebx,
            bus_mhz: r.ecx,
        }
    }

    /// Nominal/base frequency in Hz (if EAX was non-zero).
    #[inline]
    pub fn base_hz(&self) -> Option<u64> {
        (self.base_mhz != 0).then(|| (self.base_mhz as u64) * 1_000_000)
    }

    /// Maximum frequency in Hz (if EBX was non-zero).
    #[inline]
    pub fn max_hz(&self) -> Option<u64> {
        (self.max_mhz != 0).then(|| (self.max_mhz as u64) * 1_000_000)
    }

    /// Bus/reference frequency in Hz (if ECX was non-zero).
    #[inline]
    pub fn bus_hz(&self) -> Option<u64> {
        (self.bus_mhz != 0).then(|| (self.bus_mhz as u64) * 1_000_000)
    }

    /// If CPUID.15H exposed a TSC ratio but **crystal_hz==0**, you can
    /// *guess* `crystal_hz` from 16H’s bus/reference clock.
    /// Returns `Some(tsc_hz)` if both ratio and bus clock are known.
    #[inline]
    pub fn tsc_hz_from_ratio(&self, denom: u32, numer: u32) -> Option<u64> {
        if denom == 0 || numer == 0 {
            return None;
        }
        let bus = self.bus_hz()?;
        Some(bus * (numer as u64) / (denom as u64))
    }
}
