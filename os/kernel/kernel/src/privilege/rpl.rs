//! Requested Privilege Level (RPL).
//!
//! RPL lives in the **low 2 bits of a segment selector** supplied by the requester
//! (the code performing the load). Don’t confuse it with:
//! - **CPL** (Current Privilege Level): taken from the running `CS`.
//! - **DPL** (Descriptor Privilege Level): stored in the target descriptor (GDT/LDT/gate).
//!
//! For *data* segment loads, the CPU checks: `max(CPL, RPL) ≤ DPL`.
//! For `SS` loads in long mode: `CPL == RPL == DPL`.
//!
//! This module provides the `Rpl` enum plus helpers to extract/apply it to
//! a selector, and convenience checks that mirror the architecture’s rules.

#![allow(dead_code)]

use crate::privilege::Ring;

/// RPL mask in a 16-bit selector.
pub const RPL_MASK: u16 = 0b11;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Rpl {
    Ring0 = 0,
    #[allow(deprecated)]
    Ring1 = 1,
    #[allow(deprecated)]
    Ring2 = 2,
    Ring3 = 3,
}

impl Rpl {
    /// Construct from a `Ring` value.
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

    /// Convert back to a `Ring`.
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

    /// Encode as the low two bits of a selector.
    #[inline]
    #[must_use]
    pub const fn into_bits(self) -> u16 {
        self as u16
    }

    /// Decode from the low two bits.
    #[inline]
    #[must_use]
    pub const fn from_bits(value_low2: u16) -> Self {
        match value_low2 & RPL_MASK {
            0 => Self::Ring0,
            1 => Self::Ring1,
            2 => Self::Ring2,
            _ => Self::Ring3,
        }
    }

    /// Extract `RPL` from a 16-bit segment selector value.
    #[inline]
    #[must_use]
    pub const fn from_selector(selector: u16) -> Self {
        Self::from_bits(selector & RPL_MASK)
    }

    /// Return `selector` with its `RPL` bits replaced by `self`.
    #[inline]
    #[must_use]
    pub const fn apply_to_selector(self, selector: u16) -> u16 {
        (selector & !RPL_MASK) | self.into_bits()
    }

    /// Build a selector from `{index, ti}` and this `RPL`.
    ///
    /// - `index`: descriptor index in the table (GDT/LDT)
    /// - `ti`: table indicator (0 = GDT, 1 = LDT)
    ///
    /// Layout: `selector = (index << 3) | (ti << 2) | rpl`.
    #[inline]
    #[must_use]
    pub const fn build_selector(self, index: u16, ti: bool) -> u16 {
        (index << 3) | ((ti as u16) << 2) | self.into_bits()
    }

    /// The *effective requester level* that participates in data-segment checks:
    /// `max(CPL, RPL)`.
    #[inline]
    #[must_use]
    pub const fn effective_with(self, cpl: Ring) -> Ring {
        let c = cpl as u8;
        let r = self as u8;
        if c >= r { cpl } else { self.to_ring() }
    }

    /// CPU-like predicate for loading a *data* segment against a given `DPL`.
    /// Returns `true` iff `max(CPL, RPL) ≤ DPL`.
    ///
    /// (Keep this here for tests/diagnostics; your production check may live
    /// next to `Dpl` to avoid cyclic deps.)
    #[inline]
    #[must_use]
    pub const fn can_load_data_with_dpl(self, cpl: Ring, dpl: Ring) -> bool {
        (self.effective_with(cpl) as u8) <= (dpl as u8)
    }
}

impl From<Ring> for Rpl {
    #[inline]
    fn from(ring: Ring) -> Self {
        Self::from_ring(ring)
    }
}

impl From<Rpl> for Ring {
    #[inline]
    fn from(rpl: Rpl) -> Self {
        rpl.to_ring()
    }
}

pub const KERNEL_RPL: Rpl = Rpl::Ring0;
pub const USER_RPL: Rpl = Rpl::Ring3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpl_bits_roundtrip() {
        for b in 0u16..=3 {
            let r = Rpl::from_bits(b);
            assert_eq!(r.into_bits(), b);
        }
    }

    #[test]
    fn selector_pack_unpack() {
        let base: u16 = (0x1234 & !RPL_MASK); // pretend this is some CS without RPL
        let with_user = USER_RPL.apply_to_selector(base);
        assert_eq!(Rpl::from_selector(with_user), USER_RPL);
        let with_kernel = KERNEL_RPL.apply_to_selector(base);
        assert_eq!(Rpl::from_selector(with_kernel), KERNEL_RPL);
        // Make sure only low 2 bits changed:
        assert_eq!(with_user & !RPL_MASK, base & !RPL_MASK);
        assert_eq!(with_kernel & !RPL_MASK, base & !RPL_MASK);
    }

    #[test]
    fn build_selector_gdt_ldt() {
        let s_gdt = USER_RPL.build_selector(5, false);
        let s_ldt = USER_RPL.build_selector(5, true);
        assert_eq!(Rpl::from_selector(s_gdt), USER_RPL);
        assert_eq!(Rpl::from_selector(s_ldt), USER_RPL);
        assert_eq!((s_gdt >> 3), 5);
        assert_eq!((s_ldt >> 3), 5);
        assert_eq!(((s_gdt >> 2) & 1), 0); // GDT
        assert_eq!(((s_ldt >> 2) & 1), 1); // LDT
    }

    #[test]
    fn effective_and_check() {
        // CPL=0, RPL=3 → effective=3, can load only if DPL≥3
        assert_eq!(USER_RPL.effective_with(Ring::Ring0), Ring::Ring3);
        assert!(!USER_RPL.can_load_data_with_dpl(Ring::Ring0, Ring::Ring0));
        assert!(USER_RPL.can_load_data_with_dpl(Ring::Ring0, Ring::Ring3));

        // CPL=3, RPL=0 → effective=3 (max), same result.
        assert_eq!(KERNEL_RPL.effective_with(Ring::Ring3), Ring::Ring3);
    }
}
