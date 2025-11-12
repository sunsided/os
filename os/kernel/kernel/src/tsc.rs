#![allow(dead_code)]

use crate::cpuid::{CpuidRanges, Leaf15h, Leaf16};
use crate::ports::{inb, outb};

/// Best-effort TSC frequency estimate in Hz.
/// Order: CPUID.15H → CPUID.16H → PIT measurement.
/// Call with interrupts masked to reduce jitter during PIT timing.
pub unsafe fn estimate_tsc_hz() -> u64 {
    unsafe {
        if let Some(hz) = cpuid_leaf_15_tsc_hz() {
            return hz;
        }
        if let Some(hz) = cpuid_leaf_16_base_mhz_hz() {
            return hz;
        }
        pit_measure_tsc_hz(50_000) // 50 ms window for decent precision
    }
}

/// Try CPUID.15H (TSC/CORE crystal + ratio).
/// EAX = denom, EBX = numer, ECX = `crystal_hz` (may be 0).
#[inline]
unsafe fn cpuid_leaf_15_tsc_hz() -> Option<u64> {
    // Check max leaf first.
    let ranges = unsafe { CpuidRanges::read() };
    let r = unsafe { Leaf15h::read(&ranges)? };

    if r.denom == 0 || r.numer == 0 {
        return None;
    }
    // If ECX==0, many VMs/older CPUs don’t report the crystal. Bail out here;
    // avoiding guessing (19.2/24/25 MHz) keeps this robust.
    if r.crystal_hz == 0 {
        return None;
    }
    // TSC Hz = crystal_hz * (num / den)
    Some(
        u64::from(r.crystal_hz)
            .saturating_mul(u64::from(r.numer))
            .saturating_div(u64::from(r.denom)),
    )
}

/// Try CPUID.16H (processor base frequency in MHz).
/// EAX = base MHz (may be 0 on many VMs).
#[inline]
unsafe fn cpuid_leaf_16_base_mhz_hz() -> Option<u64> {
    let ranges = unsafe { CpuidRanges::read() };
    let r = unsafe { Leaf16::read(&ranges)? };
    if r.base_mhz == 0 {
        return None;
    }

    // Treat base MHz as TSC MHz. Often true under KVM/QEMU, good enough for a first pass.
    Some(u64::from(r.base_mhz) * 1_000_000u64)
}

/// Calibrate TSC Hz by measuring rdtsc delta over a PIT window.
/// Uses PIT channel 0 in mode 2 (rate generator).
/// `window_us` typically `10_000–100_000`; larger → better precision.
unsafe fn pit_measure_tsc_hz(window_us: u64) -> u64 {
    const PIT_CH0_DATA: u16 = 0x40;
    const PIT_CMD: u16 = 0x43;
    const PIT_INPUT_HZ: u64 = 1_193_182;

    // Compute PIT ticks for requested window (mode 2 expects 16-bit reload; clamp to >=1).
    let desired_ticks = (PIT_INPUT_HZ * window_us).div_ceil(1_000_000);
    let reload = desired_ticks.clamp(1, 0xFFFF) as u16;

    // Program PIT: Channel 0, Access lobyte/hibyte, Mode 2 (rate gen), Binary
    unsafe {
        outb(PIT_CMD, 0b0011_0100);
        outb(PIT_CH0_DATA, (reload & 0x00FF) as u8);
        outb(PIT_CH0_DATA, (reload >> 8) as u8);
    }

    // Latch TSC, then busy-wait roughly window_us using a software delay
    // driven by the PIT countdown. We poll the PIT output by reading back the
    // counter to approximate the elapsed time without needing IRQ0.
    let t0 = rdtsc();
    unsafe {
        busy_wait_pit_window(reload);
    }
    let t1 = rdtsc();

    // Convert TSC delta to Hz using window_us
    let delta = t1.saturating_sub(t0);
    if window_us == 0 {
        return 0;
    }
    (delta.saturating_mul(1_000_000)) / window_us
}

/// Spin until the PIT has counted down approximately one reload period in mode 2.
/// We read back the counter by issuing a latch command; this is coarse but stable
/// enough for a one-shot window.
#[allow(clippy::missing_transmute_annotations)]
unsafe fn busy_wait_pit_window(reload: u16) {
    // Latch the count repeatedly and exit once it wraps near zero.
    // In mode 2 the counter reloads on terminal count; we wait for a wrap.
    let mut last = unsafe { read_pit_counter() };
    loop {
        let cur = unsafe { read_pit_counter() };
        // Detect wrap: counter increases (since it reloads), or falls below small threshold.
        if cur > last || cur <= 2 || cur >= reload - 2 {
            break;
        }
        last = cur;
        cpu_relax();
    }

    // Small extra settle to reduce sampling jitter.
    for _ in 0..1024 {
        unsafe {
            core::arch::asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}

#[inline]
unsafe fn read_pit_counter() -> u16 {
    const PIT_CH0_DATA: u16 = 0x40;
    const PIT_CMD: u16 = 0x43;

    // Latch channel 0 count
    unsafe {
        outb(PIT_CMD, 0b0000_0000);
        let lo = u16::from(inb(PIT_CH0_DATA));
        let hi = u16::from(inb(PIT_CH0_DATA));
        (hi << 8) | lo
    }
}

/// Allows the CPU to save some power. Functionally equivalent to [`spin_loop`](core::hint::spin_loop).
#[inline(always)]
#[allow(clippy::inline_always)]
fn cpu_relax() {
    // Okay to call without unsafe
    unsafe {
        core::arch::asm!("pause", options(nomem, nostack, preserves_flags));
    }
}

#[inline(always)]
#[allow(clippy::inline_always)]
pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!(
            "lfence", // serialize (Intel-recommended)
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack, preserves_flags),
        );
    }
    (u64::from(hi) << 32) | u64::from(lo)
}
