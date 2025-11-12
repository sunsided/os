use crate::cpuid::Leaf01h;
use crate::interrupts::spurious::SPURIOUS_INTERRUPT_VECTOR;
use crate::interrupts::timer::LAPIC_TIMER_VECTOR;
use crate::per_cpu::PerCpu;
use crate::tsc::rdtsc;
use log::info;

// IA32_APIC_BASE MSR and bits
pub const IA32_APIC_BASE: u32 = 0x1B;
const APIC_EN: u64 = 1 << 11; // APIC global enable
const APIC_EXTD: u64 = 1 << 10; // x2APIC mode

// x2APIC MSRs
const IA32_X2APIC_ID: u32 = 0x802;
const IA32_X2APIC_EOI: u32 = 0x80B;
const IA32_X2APIC_SVR: u32 = 0x80F;
const IA32_X2APIC_LVT_TIMER: u32 = 0x832;
const IA32_X2APIC_INITCNT: u32 = 0x838;
const IA32_X2APIC_DIVCONF: u32 = 0x83E;

#[inline]
unsafe fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack)
        );
    }
    (u64::from(hi) << 32) | u64::from(lo)
}

#[inline]
#[allow(clippy::cast_possible_truncation)]
unsafe fn wrmsr(msr: u32, val: u64) {
    let lo = (val & 0xFFFF_FFFF) as u32;
    let hi = (val >> 32) as u32;
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") lo,
            in("edx") hi,
            options(nostack)
        );
    }
}

/// Enable x2APIC and return the Local APIC ID.
/// Panics if x2APIC isn’t supported.
pub unsafe fn enable_and_read_id_x2apic() -> u32 {
    let has_x2apic = unsafe { Leaf01h::new().has_x2apic() };
    assert!(has_x2apic, "x2APIC not supported on this CPU/VM");

    let mut base = unsafe { rdmsr(IA32_APIC_BASE) };

    // Enter x2APIC mode and enable the APIC.
    base |= APIC_EN | APIC_EXTD;
    unsafe { wrmsr(IA32_APIC_BASE, base) };

    // Optionally assert we’re really in x2APIC.
    let now = unsafe { rdmsr(IA32_APIC_BASE) };
    debug_assert!((now & APIC_EXTD) != 0, "failed to set x2APIC bit");

    // Read APIC ID via MSR.
    x2apic_id()
}

#[inline]
#[allow(clippy::cast_possible_truncation)]
pub fn x2apic_id() -> u32 {
    let val: u64 = unsafe { rdmsr(IA32_X2APIC_ID) };
    (val & 0xFFFF_FFFF) as u32
}

/// Set the Spurious Interrupt Vector register and software-enable the LAPIC.
pub unsafe fn write_svr_x2apic(vector: u8) {
    // SVR bit 8 = APIC software enable; low 8 bits = vector
    let val = (1u64 << 8) | u64::from(vector);
    unsafe {
        wrmsr(IA32_X2APIC_SVR, val);
    }
}

/// Signal End-of-Interrupt.
pub unsafe fn eoi_x2apic() {
    unsafe {
        wrmsr(IA32_X2APIC_EOI, 0);
    }
}

/// Program the LAPIC timer in periodic mode (x2APIC).
///
/// `divider` uses the LAPIC divide configuration encoding.
/// `initial` is the initial counter value in APIC ticks.
pub unsafe fn program_timer_periodic_x2apic(vector: u8, divider: u32, initial: u32) {
    let mut lvt = u64::from(vector) | (1u64 << 16); // mask bit
    lvt |= 1u64 << 17; // periodic

    unsafe {
        wrmsr(IA32_X2APIC_LVT_TIMER, lvt);
        wrmsr(IA32_X2APIC_INITCNT, u64::from(initial));
        wrmsr(IA32_X2APIC_DIVCONF, u64::from(divider));

        // Unmask to arm delivery.
        mask_timer_x2apic(false);
    }
}

#[allow(clippy::cast_possible_truncation)]
pub unsafe fn mask_timer_x2apic(mask: bool) {
    const IA32_X2APIC_LVT_TIMER: u32 = 0x832;
    let mut lvt = unsafe { rdmsr(IA32_X2APIC_LVT_TIMER) as u32 };
    if mask {
        lvt |= 1 << 16;
    } else {
        lvt &= !(1 << 16);
    }
    unsafe { wrmsr(IA32_X2APIC_LVT_TIMER, u64::from(lvt)) };
}

/// Bring up x2APIC on the BSP and record the APIC ID in `PerCpu`.
pub fn init_lapic_and_set_cpu_id(percpu: &mut PerCpu) {
    info!("Initializing LAPIC (x2APIC)…");
    let apic_id = unsafe { enable_and_read_id_x2apic() };
    percpu.apic_id = apic_id;

    lapic_enable_spurious_vector();
    info!("x2APIC enabled; APIC ID = {apic_id:#x}");
}

fn lapic_enable_spurious_vector() {
    // Choose a spurious vector (>= 0x10, unused).
    unsafe { write_svr_x2apic(SPURIOUS_INTERRUPT_VECTOR) };
}

#[allow(dead_code)]
pub mod lapic_div {
    pub const DIV_1: u32 = 0b1011;
    pub const DIV_2: u32 = 0b0000;
    pub const DIV_4: u32 = 0b0001;
    pub const DIV_8: u32 = 0b0010;
    pub const DIV_16: u32 = 0b0011;
    pub const DIV_32: u32 = 0b1000;
    pub const DIV_64: u32 = 0b1001;
    pub const DIV_128: u32 = 0b1010;
}

/// Quick helper to start a periodic timer (coarse values; calibrate later).
#[allow(clippy::cast_possible_truncation)]
pub fn start_lapic_timer(tsc_hz: u64) {
    // Make sure: SVR enabled, TPR=0, IF=1, IDT has the gate.
    unsafe {
        // Calibrate once (cache result).
        info!("Calibrating LAPIC timer via TSC ...");
        let lapic_hz = calibrate_lapic_hz_via_tsc(tsc_hz, 100_000, lapic_div::DIV_16); // 50ms, /16

        // Choose rate & compute initial
        let target_hz = 1_000u64; // 1 kHz
        let div = lapic_div::DIV_16;
        let dec_rate = lapic_hz / 16;
        let initial = (dec_rate / target_hz) as u32;

        // Arm periodic
        program_timer_periodic_x2apic(LAPIC_TIMER_VECTOR, div, initial);
    }
}

#[allow(clippy::cast_possible_truncation)]
unsafe fn calibrate_lapic_hz_via_tsc(tsc_hz: u64, window_us: u64, div: u32) -> u64 {
    // Program LAPIC masked at chosen divider
    const LVT: u32 = 0x832;
    const DIV: u32 = 0x83E;
    const INIT: u32 = 0x838;
    unsafe {
        wrmsr(DIV, u64::from(div));
    }

    // mask, vector don't matter for calibration
    let lvt = (1u64 << 16) | 0xFF;
    unsafe {
        wrmsr(LVT, lvt);
    }

    // Start from max
    unsafe {
        wrmsr(INIT, 0xFFFF_FFFF);
    }

    // Busy-wait for window_us using TSC
    let start = rdtsc();
    let target = start + (tsc_hz / 1_000_000) * window_us;
    while rdtsc() < target {}

    // Read CURRCNT and compute elapsed ticks
    let cur = unsafe { rdmsr(0x839) as u32 };
    let elapsed = 0xFFFF_FFFFu64 - u64::from(cur); // ticks at (lapic_hz/div)

    // Convert to Hz: elapsed ticks happened in window_us
    let ticks_per_sec = elapsed * 1_000_000 / window_us;

    // That equals (lapic_hz / div). Multiply back:
    #[allow(clippy::match_same_arms)]
    let multiplier = match div {
        0b1011 => 1,
        0b0000 => 2,
        0b0001 => 4,
        0b0010 => 8,
        0b0011 => 16,
        0b1000 => 32,
        0b1001 => 64,
        0b1010 => 128,
        _ => 16, // default
    };

    ticks_per_sec * multiplier
}
