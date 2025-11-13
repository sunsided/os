use bitfield_struct::bitfield;
use kernel_memory_addresses::VirtualAddress;

/// Low 8 bytes of a 64-bit *Available TSS* descriptor (type = 0x9, S=0).
#[bitfield(u64)]
pub struct TssDescLow {
    pub limit_lo: u16, // [15:0]
    pub base_lo: u16,  // [31:16]

    pub base_mid: u8, // [39:32]
    #[bits(4)]
    pub typ: u8, // [43:40] = 0x9 (Available 64-bit TSS)
    pub s: bool,      // [44]    = 0 (system)
    #[bits(2)]
    pub dpl: u8, // [46:45] typically 0
    pub p: bool,      // [47]    = 1

    #[bits(4)]
    pub limit_hi: u8, // [51:48]
    pub avl: bool,   // [52]    = 0
    pub zero1: bool, // [53]    = 0 (must be 0 for system types)
    pub zero2: bool, // [54]    = 0 (must be 0 for system types)
    pub g: bool,     // [55] granularity (keep 0 for byte granularity)
    pub base_hi: u8, // [63:56]
}

/// High 8 bytes of a 64-bit TSS descriptor: `base[63:32]`, reserved=0.
#[bitfield(u64)]
pub struct TssDescHigh {
    pub base_upper: u32, // [31:0]  base[63:32]
    reserved: u32,       // [63:32] must be 0
}

/// 16-byte TSS system descriptor (two consecutive GDT entries).
#[repr(C)]
#[derive(Copy, Clone)]
pub struct TssDesc64 {
    pub low: TssDescLow,
    pub high: TssDescHigh,
}

impl TssDesc64 {
    /// Build a 64-bit *Available TSS* descriptor (type 0x9).
    #[inline]
    pub const fn new(tss_base: VirtualAddress, tss_limit: u32) -> Self {
        let limit_lo = (tss_limit & 0xFFFF) as u16;
        let limit_hi = ((tss_limit >> 16) & 0xF) as u8;

        let base_lo = (tss_base.as_u64() & 0xFFFF) as u16;
        let base_mid = ((tss_base.as_u64() >> 16) & 0xFF) as u8;
        let base_hi = ((tss_base.as_u64() >> 24) & 0xFF) as u8;
        let base_upper = (tss_base.as_u64() >> 32) as u32;

        let low = TssDescLow::new()
            .with_limit_lo(limit_lo)
            .with_base_lo(base_lo)
            .with_base_mid(base_mid)
            .with_typ(0x9) // Available 64-bit TSS
            .with_s(false) // system descriptor
            .with_dpl(0)
            .with_p(true)
            .with_limit_hi(limit_hi)
            .with_avl(false)
            .with_zero1(false)
            .with_zero2(false)
            .with_g(false)
            .with_base_hi(base_hi);

        let high = TssDescHigh::new().with_base_upper(base_upper);

        Self { low, high }
    }
}

const _: () = {
    use core::mem::size_of;
    assert!(size_of::<TssDescLow>() == 8);
    assert!(size_of::<TssDescHigh>() == 8);
    assert!(size_of::<TssDesc64>() == 16);
};
