//! # 64-bit GDT code/data descriptor encodings (typed builders)
//!
//! In long mode, **base** and **limit** fields of *code/data* descriptors are
//! ignored for linear address calculation; paging provides memory protection.
//! What **does** matter is:
//! - **Type** (code vs data, readable/writable),
//! - **S** (descriptor class: code/data vs system),
//! - **DPL** (descriptor privilege level),
//! - **P** (present),
//! - **L** (64-bit code enable for CS),
//! - **DB** (must be 0 for 64-bit code segments),
//! - **G** (granularity; irrelevant here since limit is ignored).
//!
//! This module provides bitfield views plus a safe `Desc64` wrapper with
//! constructors that set the correct invariants for **64-bit code** and **data**
//! segments, so you don’t have to twiddle bits by hand.

use bitfield_struct::bitfield;

/// Bit layout of a **64-bit code segment** descriptor.
///
/// Invariants enforced by `from_code_dpl`:
/// - `typ = 0b1010` (execute + read),
/// - `s = 1` (code/data),
/// - `l = 1` (64-bit code),
/// - `db = 0` (required when `l = 1`),
/// - `p = 1` (present),
/// - `limit`/`base` are zero (ignored by the CPU for code in long mode).
#[bitfield(u64)]
pub struct CodeDescBits {
    pub limit_lo: u16, // [15:0]   (ignored in long mode)
    pub base_lo: u16,  // [31:16]  (ignored in long mode)
    pub base_mid: u8,  // [39:32]
    #[bits(4)]
    pub typ: u8, // [43:40] = 0b1010 (exec+read)
    pub s: bool,       // [44]     = 1 (code/data)
    #[bits(2)]
    pub dpl: u8, // [46:45]  = 0 or 3
    pub p: bool,       // [47]     = 1
    #[bits(4)]
    pub limit_hi: u8, // [51:48]
    pub avl: bool,     // [52]
    pub l: bool,       // [53]     = 1 (64-bit code)
    pub db: bool,      // [54]     = 0 when L=1
    pub g: bool,       // [55]
    pub base_hi: u8,   // [63:56]
}

/// Bit layout of a **data/stack segment** descriptor.
///
/// Invariants enforced by `from_data_dpl`:
/// - `typ = 0b0010` (read/write data),
/// - `s = 1`, `l = 0`, `p = 1`,
/// - `db` is set to `0` here (commonly set to 1 for 32-bit data, but DB has no
///   meaning for 64-bit data segments used as DS/ES/SS in long mode).
#[bitfield(u64)]
pub struct DataDescBits {
    pub limit_lo: u16, // [15:0]
    pub base_lo: u16,  // [31:16]
    pub base_mid: u8,  // [39:32]
    #[bits(4)]
    pub typ: u8, // [43:40] = 0b0010 (read/write data)
    pub s: bool,       // [44]     = 1
    #[bits(2)]
    pub dpl: u8, // [46:45]
    pub p: bool,       // [47]     = 1
    #[bits(4)]
    pub limit_hi: u8, // [51:48]
    pub avl: bool,     // [52]
    pub l: bool,       // [53]     = 0 for data
    pub db: bool,      // [54]
    pub g: bool,       // [55]
    pub base_hi: u8,   // [63:56]
}

/// A single 8-byte GDT entry with **code** or **data** view.
///
/// Use the safe constructors to create valid 64-bit descriptors. The `raw` view
/// is provided for table emission; reading the wrong structured view is UB.
#[repr(C)]
#[derive(Copy, Clone)]
pub union Desc64 {
    pub raw: u64,
    pub code: CodeDescBits,
    pub data: DataDescBits,
}

impl Desc64 {
    /// Build a **64-bit code** descriptor (execute+read, `L=1`, `DB=0`).
    ///
    /// `dpl` must be in `0..=3` (masked internally).
    pub const fn from_code_dpl(dpl: u8) -> Self {
        let code = CodeDescBits::new()
            .with_limit_lo(0)
            .with_base_lo(0)
            .with_base_mid(0)
            .with_typ(0b1010)
            .with_s(true)
            .with_dpl(dpl & 0b11)
            .with_p(true)
            .with_limit_hi(0)
            .with_avl(false)
            .with_l(true) // 64-bit code
            .with_db(false) // must be 0 with L=1
            .with_g(false)
            .with_base_hi(0);
        Self { code }
    }

    /// Build a **data/stack** descriptor (read/write, `L=0`).
    ///
    /// `dpl` must be in `0..=3` (masked internally).
    pub const fn from_data_dpl(dpl: u8) -> Self {
        let data = DataDescBits::new()
            .with_limit_lo(0)
            .with_base_lo(0)
            .with_base_mid(0)
            .with_typ(0b0010)
            .with_s(true)
            .with_dpl(dpl & 0b11)
            .with_p(true)
            .with_limit_hi(0)
            .with_avl(false)
            .with_l(false)
            .with_db(false)
            .with_g(false)
            .with_base_hi(0);
        Self { data }
    }

    /// Raw 64-bit encoding (safe to read for either variant).
    #[inline]
    pub const fn to_u64(self) -> u64 {
        // Reading the `raw` field is always valid.
        unsafe { self.raw }
    }

    /// View as code bits. **Unsafe**: UB if this entry isn’t a code descriptor.
    #[inline]
    pub const unsafe fn as_code(self) -> CodeDescBits {
        unsafe { self.code }
    }

    /// View as data bits. **Unsafe**: UB if this entry isn’t a data descriptor.
    #[inline]
    pub const unsafe fn as_data(self) -> DataDescBits {
        unsafe { self.data }
    }
}

// Size guards: each descriptor is exactly 8 bytes.
const _: () = {
    assert!(size_of::<CodeDescBits>() == 8);
    assert!(size_of::<DataDescBits>() == 8);
    assert!(size_of::<Desc64>() == 8);
};
