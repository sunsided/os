use crate::privilege::{Ring, Rpl};

/// Descriptor Privilege Level (lives *in the descriptor*).
///
/// Do not confuse with:
/// - `CPL` (Current Privilege Level) — taken from `CS` of the running code
/// - `RPL` (Requested Privilege Level) — low 2 bits in a segment selector
///
/// Typical checks the CPU enforces include:
/// - Data segments: `max(CPL, RPL) ≤ DPL`
/// - Non-conforming code: `CPL == DPL`
/// - Conforming code: `CPL ≤ DPL`
/// - `SS` load (long mode): `CPL == RPL == DPL`
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Dpl {
    Ring0 = 0,
    #[allow(deprecated)]
    Ring1 = 1,
    #[allow(deprecated)]
    Ring2 = 2,
    Ring3 = 3,
}

impl Dpl {
    #[inline]
    #[must_use]
    pub const fn from_ring(ring: Ring) -> Self {
        match ring {
            Ring::Ring0 => Self::Ring0,
            #[allow(deprecated)]
            Ring::Ring1 => Self::Ring1,
            #[allow(deprecated)]
            Ring::Ring2 => Self::Ring2,
            Ring::Ring3 => Self::Ring3,
        }
    }

    #[inline]
    #[must_use]
    pub const fn to_ring(self) -> Ring {
        match self {
            Self::Ring0 => Ring::Ring0,
            #[allow(deprecated)]
            Self::Ring1 => Ring::Ring1,
            #[allow(deprecated)]
            Self::Ring2 => Ring::Ring2,
            Self::Ring3 => Ring::Ring3,
        }
    }

    #[inline]
    pub const fn into_bits(self) -> u8 {
        self as u8
    }

    #[inline]
    pub const fn from_bits(v: u8) -> Self {
        match v & 0b11 {
            0 => Self::Ring0,
            1 => Self::Ring1,
            2 => Self::Ring2,
            _ => Self::Ring3,
        }
    }

    /// Data segment: allowed iff `max(CPL, RPL) ≤ DPL`.
    #[inline]
    #[must_use]
    pub const fn permits_data_load(self, cpl: Ring, rpl: Rpl) -> bool {
        (rpl.effective_with(cpl) as u8) <= (self as u8)
    }

    /// Non-conforming code: `CPL == DPL`.
    #[inline]
    #[must_use]
    pub const fn permits_nonconforming_code(self, cpl: Ring) -> bool {
        (self as u8) == (cpl as u8)
    }

    /// Conforming code: `CPL ≤ DPL`.
    #[inline]
    #[must_use]
    pub const fn permits_conforming_code(self, cpl: Ring) -> bool {
        (cpl as u8) <= (self as u8)
    }

    /// Stack segment load in long mode: `CPL == RPL == DPL`.
    #[inline]
    #[must_use]
    pub const fn permits_ss_load_longmode(self, cpl: Ring, rpl: Rpl) -> bool {
        (self as u8) == (cpl as u8) && (cpl as u8) == (rpl as u8)
    }
}

impl From<Ring> for Dpl {
    #[inline]
    fn from(ring: Ring) -> Self {
        Self::from_ring(ring)
    }
}

impl From<Dpl> for Ring {
    #[inline]
    fn from(dpl: Dpl) -> Self {
        dpl.to_ring()
    }
}
