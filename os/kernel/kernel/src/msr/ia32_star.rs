use crate::gdt::Selectors;
use crate::gdt::selectors::{CodeSel, SegmentSelector};
use crate::privilege::Rpl;
use core::borrow::Borrow;
use kernel_registers::msr::Ia32Star;

/// `IA32_STAR` — System Call Target & Segment Selectors (MSR `0xC000_0081`).
///
/// In 64-bit mode:
///
/// - `syscall` uses `IA32_STAR[47:32]` to derive kernel CS/SS.
/// - `sysret` uses `IA32_STAR[63:48]` to derive user CS/SS.
/// - `IA32_LSTAR` provides the 64-bit RIP target for `syscall`.
///
/// In compatibility mode, `IA32_STAR[31:0]` holds the 32-bit EIP target for `syscall`.
pub trait Ia32StarExt {
    fn from_selectors<T>(selector: T) -> Self
    where
        T: Borrow<Selectors>;

    /// Helper to build a STAR value for a pure 64-bit kernel.
    ///
    /// `kernel_cs` and `user_cs` are the *selectors* (e.g. `0x08` and `0x1b`).
    fn new_64bit(kernel_cs: SegmentSelector<CodeSel>, user_cs: SegmentSelector<CodeSel>) -> Self;
}

impl Ia32StarExt for Ia32Star {
    fn from_selectors<T>(selector: T) -> Self
    where
        T: Borrow<Selectors>,
    {
        let selector = selector.borrow();
        <Self as Ia32StarExt>::new_64bit(selector.kernel_cs, selector.user_cs)
    }

    /// Helper to build a STAR value for a pure 64-bit kernel.
    ///
    /// `kernel_cs` and `user_cs` are the *selectors* (e.g. `0x08` and `0x1b`).
    fn new_64bit(kernel_cs: SegmentSelector<CodeSel>, user_cs: SegmentSelector<CodeSel>) -> Self {
        // Assumption:
        // kcode -> kdata -> udata -> ucode

        // Extract and validate GDT indices
        let kidx = kernel_cs.index();
        let uidx = user_cs.index();

        // SYSRET requires:
        //   user_ss_index = user_cs_index - 1
        // because SS uses (base + 8), CS uses (base + 16).
        debug_assert_ne!(kidx, 0);
        debug_assert_eq!(kernel_cs.rpl(), Rpl::Ring0);
        debug_assert_ne!(uidx, 0, "User CS selector at GDT index 0 is invalid");

        Self::new_64bit_raw(kernel_cs.into_bits(), user_cs.into_bits())
    }
}
