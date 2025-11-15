use bitfield_struct::bitfield;

/// Architectural RFLAGS model for x86-64.
///
/// Bits that are architecturally fixed in 64-bit mode are modeled as
/// `#[bits(default=.., access=RO)]` so they cannot be changed.
///
/// Bits that may legally be manipulated (and masked via `IA32_FMASK`)
/// are exposed as normal read/write bools or small enums.
#[bitfield(u64, order = Lsb)]
pub struct Rflags {
    /// Carry Flag
    pub cf_carry: bool, // 0

    /// Always 1 in 64-bit mode.
    #[bits(default = true)]
    _always1: bool, // 1

    /// Parity Flag
    pub pf_parity: bool, // 2

    /// Reserved (always 0)
    #[bits(default = false)]
    _rsvd3: bool, // 3

    /// Adjust Flag
    pub af_adjust: bool, // 4

    /// Reserved (always 0)
    #[bits(default = false)]
    _rsvd5: bool, // 5

    /// Zero Flag
    pub zf_zero: bool, // 6

    /// Sign Flag
    pub sf_sign: bool, // 7

    /// Trap Flag
    pub tf_trap: bool, // 8

    /// Interrupt Enable Flag
    pub if_interrupt_enable: bool, // 9

    /// Direction Flag
    pub df_direction: bool, // 10

    /// Overflow Flag
    pub of_overflow: bool, // 11

    /// I/O Privilege Level (2 bits)
    #[bits(2)]
    pub iopl: u8, // 12–13

    /// Nested Task
    pub nt_nested: bool, // 14

    /// Reserved (always 0 in x86-64)
    #[bits(default = false)]
    _rsvd15: bool, // 15

    /// Resume Flag
    pub rf_resume: bool, // 16

    /// Virtual 8086 mode — must be 0 in 64-bit mode.
    #[bits(default = false)]
    _vm: bool, // 17

    /// Alignment Check
    pub ac_alignment_check: bool, // 18

    /// Virtual Interrupt Flag
    pub vif_virtual_interrupt: bool, // 19

    /// Virtual Interrupt Pending
    pub vip_virtual_interrupt_pending: bool, // 20

    /// ID Flag: allows toggling CPUID.
    pub id_cpuid: bool, // 21

    /// Reserved 22–63 (all zero)
    #[bits(42, default = false)]
    _reserved_rest: u64,
}
