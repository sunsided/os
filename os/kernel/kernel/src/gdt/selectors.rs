//! # Strongly-typed segment selectors for long mode
//!
//! Segment selectors are 16-bit values loaded into CS/DS/ES/SS (and TR for TSS).
//! Even in long mode they carry **RPL** and choose **GDT vs LDT**, and their
//! **index** selects a GDT entry. A selector encodes:
//!
//! ```text
//!  15            3 2  1  0
//! +----------------+--+----+
//! |   Index[12:0]  |TI| RPL|
//! +----------------+--+----+  (TI=0 → GDT, TI=1 → LDT; RPL=0..3)
//! ```
//!
//! This module adds a thin type layer so you can’t accidentally put a data
//! selector into CS or a random value into `ltr`. It also exposes the **raw**
//! encoding when you need to write `u16` values into an iret frame or asm.

use crate::privilege::Rpl;
use bitfield_struct::bitfield;

/// Which descriptor table a selector addresses.
///
/// Only the GDT is used here; LDT is provided for completeness.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Table {
    /// Global Descriptor Table
    Gdt = 0,
    /// Local Descriptor Table
    Ldt = 1,
}

impl Table {
    #[inline]
    pub const fn from_bits(bits: u8) -> Self {
        if bits == 0 { Self::Gdt } else { Self::Ldt }
    }

    #[inline]
    pub const fn into_bits(self) -> u8 {
        self as u8
    }
}

/// Raw 16-bit selector encoding (index/TI/RPL).
///
/// Use the typed `SegmentSelector<K>` wrappers unless you truly need the bits.
#[bitfield(u16)]
#[derive(Eq, PartialEq)]
pub struct SegmentSelectorRaw {
    /// Requested Privilege Level (bits 0..1).
    #[bits(2)]
    rpl: Rpl,
    /// Table Indicator (bit 2): 0 = GDT, 1 = LDT.
    #[bits(1)]
    ti: Table,
    /// Descriptor index (bits 3..15).
    #[bits(13)]
    index: u16,
}

impl SegmentSelectorRaw {
    /// Create a raw selector (no semantic checks).
    #[inline]
    pub const fn new_with(index: u16, table: Table, rpl: Rpl) -> Self {
        Self::new().with_index(index).with_ti(table).with_rpl(rpl)
    }

    /// Return the selector as a plain `u16`.
    #[inline]
    pub const fn to_u16(self) -> u16 {
        self.into_bits()
    }
}

/// Marker trait for typed selectors.
pub trait SelectorKind: Copy {}

/// Code segment (CS) selector.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum CodeSel {}

/// Data/stack (DS/ES/SS) selector.
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DataSel {}

/// TSS system segment selector (for `ltr`).
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum TssSel {}

impl SelectorKind for CodeSel {}
impl SelectorKind for DataSel {}
impl SelectorKind for TssSel {}

/// Strongly-typed selector wrapper.
///
/// This prevents using a data selector where a code selector is required, etc.
/// Convert to `u16` with `.to_u16()` for use in `iret` frames or inline asm.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct SegmentSelector<K: SelectorKind>(SegmentSelectorRaw, core::marker::PhantomData<K>);

impl<K: SelectorKind> SegmentSelector<K> {
    /// Access the raw selector (index/TI/RPL).
    #[inline]
    pub const fn raw(self) -> SegmentSelectorRaw {
        self.0
    }

    /// Encode as `u16` (for `iret`, `mov ss, ax`, etc.).
    #[inline]
    pub const fn encode(self) -> u16 {
        self.0.into_bits()
    }
}

impl SegmentSelector<CodeSel> {
    /// Create a **code** selector from a GDT index and desired RPL.
    ///
    /// For user code, pass [`Rpl::Ring3`] to produce `…|3` (e.g. `0x1b`).
    #[inline]
    pub const fn new(index: u16, rpl: Rpl) -> Self {
        Self(
            SegmentSelectorRaw::new_with(index, Table::Gdt, rpl),
            core::marker::PhantomData,
        )
    }
}

impl SegmentSelector<DataSel> {
    /// Create a **data/stack** selector from a GDT index and RPL.
    ///
    /// For a user stack (SS), RPL must match CPL (Ring-3).
    #[inline]
    pub const fn new(index: u16, rpl: Rpl) -> Self {
        Self(
            SegmentSelectorRaw::new_with(index, Table::Gdt, rpl),
            core::marker::PhantomData,
        )
    }
}

impl SegmentSelector<TssSel> {
    /// Create a **TSS** selector for `ltr`. RPL is architecturally ignored.
    #[inline]
    pub const fn new(index: u16) -> Self {
        Self(
            SegmentSelectorRaw::new_with(index, Table::Gdt, Rpl::Ring0),
            core::marker::PhantomData,
        )
    }
}
