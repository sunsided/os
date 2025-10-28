use crate::addr2::PhysicalAddress;
use bitfield_struct::bitfield;

/// Represents a single 64-bit x86-64 page table entry in its raw bitfield form.
///
/// This structure models the **common superset** of fields found in all
/// four paging levels (PML4E, PDPTE, PDE, PTE). Each bit corresponds to a
/// hardware-defined flag or address field as specified by the AMD64 and
/// Intel manuals.
///
/// The type allows read/write access to individual bits without manual masking
/// or shifting, using the [`bitfield_struct`](https://docs.rs/bitfield-struct/)
/// derive.
///
/// ### Overview
/// A page table entry (PTE) may either:
/// - point to a **next-level page table**, or
/// - directly map a **physical page (leaf)** when the `large_page` (PS) bit is set.
///
/// Fields such as `dirty` and `global_translation` are meaningful only for
/// **leaf entries**, while others (e.g. `large_page`) have specific validity
/// rules depending on the table level.
///
/// ### Bit layout (canonical)
///
/// | Bits      | Name / Mnemonic   | Meaning |
/// |-----------|-------------------|----------|
/// | 0         | `P` (present)     | Valid entry if set |
/// | 1         | `RW`              | Writable if set |
/// | 2         | `US`              | User-mode accessible if set |
/// | 3         | `PWT`             | Write-through caching |
/// | 4         | `PCD`             | Disable caching |
/// | 5         | `A`               | Accessed |
/// | 6         | `D`               | Dirty (leaf only) |
/// | 7         | `PS`              | Large page flag |
/// | 8         | `G`               | Global (leaf only) |
/// | 9–11      | OS avail low      | Reserved for OS use |
/// | 12–51     | `addr`            | Physical frame bits [51:12] |
/// | 52–58     | OS avail high     | Reserved for OS use |
/// | 59–62     | `PKU` / OS use    | Protection key or OS use |
/// | 63        | `NX`              | Execute disable |
///
/// ### Notes
/// - Non-leaf entries ignore bits `D`, `G`, and `NX`.
/// - `PS` must be 0 in L4 and L1 entries; valid in L3 (1 GiB) and L2 (2 MiB).
/// - The physical address field always omits the lower 12 bits, which are
///   implicitly zero due to alignment.
/// - When PKU is not supported, bits 59–62 are reserved for OS use.
///
/// ### Example
/// ```rust
/// # use kernel_vmem::addr2::PhysicalAddress;
/// # use kernel_vmem::PageEntryBits;
/// let mut e = PageEntryBits::new();
/// e.set_present(true);
/// e.set_writable(true);
/// e.set_physical_address(PhysicalAddress::new(0x12345));
/// assert!(e.present());
/// ```
///
/// This type is typically used as part of higher-level abstractions like
/// `PageTable`, `PageDirectory`, or `AddressSpace` to manage paging structures
/// in a type-safe way.
#[bitfield(u64)]
pub struct PageEntryBits {
    /// Present (P, bit 0).
    ///
    /// Set if the entry points to a valid next-level table or a valid leaf
    /// mapping (depending on level/PS). Clear implies a not-present entry.
    pub present: bool,

    /// Writable (RW, bit 1).
    ///
    /// Set to allow writes; clear for read-only. Subject to supervisor/user
    /// checks via `user_access` (US) and CR0.WP behavior in supervisor mode.
    pub writable: bool,

    /// User/Supervisor (US, bit 2).
    ///
    /// Set to allow user-mode access; clear restricts to supervisor only.
    /// Combined with CPL and SMEP/SMAP if enabled.
    pub user_access: bool,

    /// Page Write-Through (PWT, bit 3).
    ///
    /// Set to use write-through caching; clear for write-back, when caching
    /// is enabled. Meaningful only when caching is not disabled.
    pub write_through: bool,

    /// Page Cache Disable (PCD, bit 4).
    ///
    /// Set to disable caching for this mapping; clear to allow caching.
    /// May impact performance significantly.
    pub cache_disabled: bool,

    /// Accessed (A, bit 5).
    ///
    /// Set by the CPU on first access (read/write/execute) through this entry.
    /// Software may clear it to track usage. Not a permission bit.
    pub accessed: bool,

    /// Dirty (D, bit 6) — **leaf only**.
    ///
    /// Set by the CPU on first write to a leaf mapping. Ignored for non-leaf
    /// entries (next-level pointers). Software may clear it to track writes.
    pub dirty: bool,

    /// Large Page / Page Size (PS, bit 7).
    ///
    /// For L3 (PDPTE) and L2 (PDE): when **set**, the entry is a **leaf**
    /// mapping to a 1 GiB (L3) or 2 MiB (L2) page. When **clear**, the entry
    /// points to the next-level table.
    ///
    /// For L4 (PML4E) and L1 (PTE): must be **clear** (0). In a 4 KiB PTE
    /// the architectural bit position is repurposed as **PAT**; this unified
    /// “superset” view treats it as PS=0 at L1—handle PAT separately if needed.
    pub large_page: bool,

    /// Global (G, bit 8) — **leaf only**.
    ///
    /// When set on a leaf mapping, the TLB entry is not flushed on CR3 reload,
    /// unless explicitly invalidated. Ignored for non-leaf entries.
    pub global_translation: bool,

    /// OS-available (bits 9..=11).
    ///
    /// Reserved for operating system use; hardware doesn’t interpret these.
    #[bits(3)]
    pub os_available_low: u8,

    /// Physical address bits [51:12] (bits 12..=51).
    ///
    /// Stores the page-aligned physical frame address without the low 12 bits.
    /// Reconstruct the full physical address as: `(bits << 12)`.
    /// For large pages, alignment requirements increase (2 MiB/1 GiB).
    #[bits(40)]
    phys_addr_bits_51_12: u64,

    /// OS-available (bits 52..=58).
    ///
    /// Additional operating system–defined storage; ignored by hardware.
    #[bits(7)]
    pub os_available_high: u8,

    /// Protection Key (PKU, bits 59..=62) if supported; otherwise OS use.
    ///
    /// With Intel PKU enabled, selects one of up to 16 protection keys whose
    /// access is controlled by PKRU. If PKU isn’t supported/active, hardware
    /// ignores these bits and they may be used by the OS.
    #[bits(4)]
    pub protection_key: u8,

    /// No-Execute (NX, bit 63) — Execute Disable.
    ///
    /// When set, instruction fetches are disallowed through this entry.
    /// Requires `EFER.NXE` support; otherwise the bit is reserved.
    /// Note: permissions are the intersection over the walk; a single NX in
    /// the path suffices to block execution.
    pub no_execute: bool,
}

impl PageEntryBits {
    #[inline]
    pub const fn set_physical_address(&mut self, phys: PhysicalAddress) {
        // store bits [51:12]
        self.set_phys_addr_bits_51_12(phys.as_u64() >> 12);
    }

    #[inline]
    #[must_use]
    pub const fn physical_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.phys_addr_bits_51_12() << 12)
    }

    #[inline]
    #[must_use]
    pub const fn new_common_rw() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(true)
            .with_user_access(false)
            .with_write_through(false)
            .with_cache_disabled(false)
            .with_no_execute(false)
    }

    #[inline]
    #[must_use]
    pub const fn flags_user_rx() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(false)
            .with_user_access(true)
            .with_write_through(false)
            .with_cache_disabled(false)
            .with_no_execute(false)
    }

    #[inline]
    #[must_use]
    pub const fn new_user_ro_nx() -> Self {
        Self::new()
            .with_present(true)
            .with_writable(false)
            .with_user_access(true)
            .with_no_execute(true)
    }
}
