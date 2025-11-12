use crate::cpuid::{CpuidRanges, CpuidResult, cpuid};
use bitfield_struct::bitfield;

pub const LEAF_01H: u32 = 0x01;

/// CPUID.01H — Feature Information (a.k.a. “leaf 1”).
///
/// Returns processor version info (EAX), brand/CLFLUSH/logical count/APIC ID (EBX),
/// and the classic feature flags (ECX/EDX). This wrapper parses the common
/// identification fields and exposes ECX/EDX via your bitfield views.
///
/// Reference: Intel SDM Vol. 2A, “CPUID—CPU Identification”, leaf 01H.
#[derive(Copy, Clone, Debug)]
pub struct Leaf01h {
    pub eax: Leaf1Eax,
    pub ebx: Leaf1Ebx,
    pub ecx: Leaf1Ecx,
    pub edx: Leaf1Edx,
}

impl Leaf01h {
    /// # Safety
    /// The caller must ensure that the `cpuid` instruction is available and leaf `0x01` exists.
    pub unsafe fn new() -> Self {
        unsafe {
            let r = cpuid(LEAF_01H, 0);
            Self::from(r)
        }
    }

    /// Query CPUID.01H if supported; returns `None` if `ranges` says leaf 1 is absent.
    #[inline]
    pub unsafe fn read(ranges: &CpuidRanges) -> Option<Self> {
        if !ranges.has_basic(LEAF_01H) {
            return None;
        }

        unsafe {
            let r = cpuid(LEAF_01H, 0);
            Some(Self::from(r))
        }
    }

    /// # Safety
    /// The caller must ensure that the passed [`CpuidResult`] belongs to a valid leaf `0x01` entry.
    pub const unsafe fn from(r: CpuidResult) -> Self {
        Self {
            eax: Leaf1Eax::from_bits(r.eax),
            ebx: Leaf1Ebx::from_bits(r.ebx),
            ecx: Leaf1Ecx::from_bits(r.ecx),
            edx: Leaf1Edx::from_bits(r.edx),
        }
    }

    #[inline]
    pub const fn has_x2apic(&self) -> bool {
        self.ecx.x2apic()
    }

    #[inline]
    pub const fn avx_usable(&self) -> bool {
        self.ecx.avx() && self.ecx.xsave() && self.ecx.osxsave()
    }

    #[inline]
    pub const fn initial_apic_id(&self) -> u8 {
        self.ebx.initial_apic_id()
    }

    #[inline]
    pub const fn logical_cpus_legacy(&self) -> u8 {
        self.ebx.logical_processor_count()
    }

    #[inline]
    pub fn family(&self) -> u16 {
        self.eax.effective_family()
    }

    #[inline]
    pub const fn model(&self) -> u8 {
        self.eax.effective_model()
    }

    #[inline]
    pub const fn stepping(&self) -> u8 {
        self.eax.stepping()
    }
}

/// CPUID.01H:EAX — Version Information.
///
/// Raw fields as defined by the SDM; helpers compute the *effective* model/family.
/// Reference: Intel SDM Vol. 2A, CPUID leaf 01H, EAX layout.
#[bitfield(u32)]
pub struct Leaf1Eax {
    /// Stepping ID (bits 3:0).
    #[bits(4)]
    stepping: u8,
    /// Base model (bits 7:4).
    #[bits(4)]
    model: u8,
    /// Base family (bits 11:8).
    #[bits(4)]
    family: u8,
    /// Processor type (bits 13:12).
    #[bits(2)]
    cpu_type: u8,
    /// Reserved (bits 15:14).
    #[bits(2)]
    _rsv14_15: u8,
    /// Extended model (bits 19:16).
    #[bits(4)]
    ext_model: u8,
    /// Extended family (bits 27:20).
    #[bits(8)]
    ext_family: u16,
    /// Reserved (bits 31:28).
    #[bits(4)]
    _rsv28_31: u8,
}

impl Leaf1Eax {
    /// Effective family per SDM:
    /// if base family == 0x0F → base + `ext_family`, else base.
    #[inline]
    pub fn effective_family(self) -> u16 {
        let fam = u16::from(self.family());
        if fam == 0x0F {
            fam + self.ext_family()
        } else {
            fam
        }
    }

    /// Effective model per SDM:
    /// if base family in {0x06, 0x0F} → `base_model` | (`ext_model` << 4), else `base_model`.
    #[inline]
    pub const fn effective_model(self) -> u8 {
        let fam = self.family();
        let base = self.model();
        if fam == 0x06 || fam == 0x0F {
            base | (self.ext_model() << 4)
        } else {
            base
        }
    }
}

/// CPUID.01H:EBX — Brand/CLFLUSH/Logical Count/APIC ID.
///
/// Reference: Intel SDM Vol. 2A, CPUID leaf 01H, EBX layout.
#[bitfield(u32)]
pub struct Leaf1Ebx {
    /// Brand index (bits 7:0).
    #[bits(8)]
    brand_index: u8,
    /// CLFLUSH line size in **8-byte** units (bits 15:8).
    #[bits(8)]
    clflush_line_size_8b: u8,
    /// Logical processors per package (legacy) (bits 23:16).
    #[bits(8)]
    logical_processor_count: u8,
    /// Initial APIC ID (bits 31:24).
    #[bits(8)]
    initial_apic_id: u8,
}

impl Leaf1Ebx {
    /// CLFLUSH line size in **bytes** (value * 8).
    #[inline]
    pub fn clflush_line_bytes(self) -> u16 {
        u16::from(self.clflush_line_size_8b()) * 8
    }
}

/// Feature flags returned by `CPUID.(EAX=1):ECX`.
///
/// Each bit represents the presence of a CPU feature or instruction-set extension.
/// These correspond to the ECX register bits returned by the CPUID instruction when
/// called with `EAX=1` and `ECX=0`.
///
/// Reference: Intel SDM Vol. 2A, Table 3-12 “Feature Information Returned in ECX for CPUID(01H)”
#[bitfield(u32)]
pub struct Leaf1Ecx {
    /// Streaming SIMD Extensions 3 (SSE3) instructions are supported.
    sse3: bool, // 0
    /// PCLMULQDQ (Carry-less multiply) instruction is supported.
    pclmulqdq: bool, // 1
    /// 64-bit DS area for debug store is supported.
    dtes64: bool, // 2
    /// MONITOR/MWAIT instructions are supported.
    monitor: bool, // 3
    /// CPL-qualified debug store area is supported.
    ds_cpl: bool, // 4
    /// Virtual Machine Extensions (VMX) are supported.
    vmx: bool, // 5
    /// Safer Mode Extensions (SMX) are supported.
    smx: bool, // 6
    /// Enhanced `SpeedStep` Technology (EST) is supported.
    est: bool, // 7
    /// Thermal Monitor 2 (TM2) is supported.
    tm2: bool, // 8
    /// Supplemental SSE3 (SSSE3) instructions are supported.
    ssse3: bool, // 9
    /// Context ID (CNXT-ID) feature is supported.
    cnxt_id: bool, // 10
    /// Silicon debug interface (SDBG) available.
    sdbg: bool, // 11
    /// Fused Multiply-Add (FMA) instructions are supported.
    fma: bool, // 12
    /// CMPXCHG16B instruction (Compare-and-swap 16 bytes) is supported.
    cx16: bool, // 13
    /// xTPR update control (xTPR) is supported.
    xtpr: bool, // 14
    /// Performance Debug Capability MSR (PDCM) available.
    pdcm: bool, // 15
    /// Reserved (bit 16).
    _rsv16: bool, // 16
    /// Process-context identifiers (PCID) are supported.
    pcid: bool, // 17
    /// Direct Cache Access (DCA) is supported.
    dca: bool, // 18
    /// Streaming SIMD Extensions 4.1 (SSE4.1) instructions are supported.
    sse4_1: bool, // 19
    /// Streaming SIMD Extensions 4.2 (SSE4.2) instructions are supported.
    sse4_2: bool, // 20
    /// x2APIC mode is supported (Extended APIC ID register interface).
    x2apic: bool, // 21
    /// MOVBE instruction (move with byte swap) is supported.
    movbe: bool, // 22
    /// POPCNT instruction is supported.
    popcnt: bool, // 23
    /// Local APIC supports TSC-deadline timer.
    tsc_deadline: bool, // 24
    /// AES-NI instructions are supported.
    aesni: bool, // 25
    /// XSAVE/XRSTOR processor state management instructions supported.
    xsave: bool, // 26
    /// OS has enabled XSAVE/XRSTOR via CR4.OSXSAVE.
    osxsave: bool, // 27
    /// Advanced Vector Extensions (AVX) instructions are supported.
    avx: bool, // 28
    /// 16-bit floating-point conversion instructions (F16C) supported.
    f16c: bool, // 29
    /// RDRAND instruction (hardware random number generator) is supported.
    rdrand: bool, // 30
    /// Hypervisor present (running under a hypervisor).
    hypervisor: bool, // 31
}

/// Feature flags returned by `CPUID.(EAX=1):EDX`.
///
/// Each bit represents a legacy feature flag.
/// These correspond to the EDX register bits returned by the CPUID instruction when
/// called with `EAX=1` and `ECX=0`.
///
/// Reference: Intel SDM Vol. 2A, Table 3-13 “Feature Information Returned in EDX for CPUID(01H)”
#[bitfield(u32)]
pub struct Leaf1Edx {
    /// On-chip FPU (x87 FPU) present.
    fpu: bool, // 0
    /// Virtual 8086 mode extensions (VME) supported.
    vme: bool, // 1
    /// Debugging extensions (DE) supported.
    de: bool, // 2
    /// Page-Size Extensions (PSE) supported.
    pse: bool, // 3
    /// Time-Stamp Counter (RDTSC) instruction available.
    tsc: bool, // 4
    /// Model-Specific Registers (RDMSR/WRMSR) supported.
    msr: bool, // 5
    /// Physical Address Extensions (PAE) supported.
    pae: bool, // 6
    /// Machine Check Exception (MCE) supported.
    mce: bool, // 7
    /// CMPXCHG8B instruction supported.
    cx8: bool, // 8
    /// On-chip APIC hardware present and enabled.
    apic: bool, // 9
    /// Reserved (bit 10).
    _rsv10_10: bool, // 10
    /// SYSENTER/SYSEXIT instructions supported.
    sep: bool, // 11
    /// Memory Type Range Registers (MTRR) supported.
    mtrr: bool, // 12
    /// Page Global Enable (PGE) supported.
    pge: bool, // 13
    /// Machine Check Architecture (MCA) supported.
    mca: bool, // 14
    /// Conditional Move (CMOV) instruction supported.
    cmov: bool, // 15
    /// Page Attribute Table (PAT) supported.
    pat: bool, // 16
    /// 36-bit Page Size Extension (PSE-36) supported.
    pse36: bool, // 17
    /// Processor Serial Number (PSN) available.
    psn: bool, // 18
    /// CLFLUSH instruction supported.
    clfsh: bool, // 19
    /// Reserved (bit 20).
    _rsv20_20: bool, // 20
    /// Debug Store (DS) feature supported.
    ds: bool, // 21
    /// Thermal Monitor and ACPI support.
    acpi: bool, // 22
    /// MMX technology supported.
    mmx: bool, // 23
    /// FXSAVE/FXRSTOR instructions for FPU/MMX state supported.
    fxsr: bool, // 24
    /// Streaming SIMD Extensions (SSE) supported.
    sse: bool, // 25
    /// Streaming SIMD Extensions 2 (SSE2) supported.
    sse2: bool, // 26
    /// Self-Snoop (SS) supported.
    ss: bool, // 27
    /// Hyper-Threading Technology (HTT) supported.
    htt: bool, // 28
    /// Thermal Monitor (TM) supported.
    tm: bool, // 29
    /// Reserved (bit 30).
    _rsv30_30: bool, // 30
    /// Pending Break Enable (PBE) supported.
    pbe: bool, // 31
}
