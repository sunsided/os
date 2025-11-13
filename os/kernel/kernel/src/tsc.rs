//! # Time Stamp Counter (TSC) Management
//!
//! This module provides comprehensive TSC (Time Stamp Counter) frequency detection
//! and measurement capabilities for the kernel's timing subsystem. The TSC is a
//! critical timing source in modern x86-64 systems, offering high-resolution
//! timestamps for kernel scheduling, profiling, and time-sensitive operations.
//!
//! ## Overview
//!
//! The Time Stamp Counter is a 64-bit register that increments at a fixed frequency
//! (typically the CPU's crystal oscillator frequency) on modern processors. This
//! module implements multiple strategies to accurately determine the TSC frequency,
//! enabling precise timing calculations throughout the kernel.
//!
//! ## TSC Frequency Detection Strategy
//!
//! The module employs a fallback hierarchy to determine TSC frequency with maximum
//! accuracy and compatibility:
//!
//! ### 1. CPUID Leaf 15H (Primary Method)
//! - **Source**: Architectural frequency information from processor
//! - **Accuracy**: Exact crystal oscillator frequency and ratio
//! - **Requirements**: TSC/CORE crystal ratio and crystal frequency both non-zero
//! - **Formula**: `TSC_Hz = crystal_hz × (numerator / denominator)`
//! - **Availability**: Modern Intel processors, some AMD processors
//!
//! ### 2. CPUID Leaf 16H (Secondary Method)
//! - **Source**: Processor base frequency information
//! - **Accuracy**: Good approximation assuming TSC runs at base frequency
//! - **Requirements**: Base frequency must be reported (non-zero)
//! - **Formula**: `TSC_Hz = base_mhz × 1,000,000`
//! - **Availability**: Intel processors with frequency reporting
//!
//! ### 3. PIT Calibration (Fallback Method)
//! - **Source**: Measurement against Programmable Interval Timer (PIT)
//! - **Accuracy**: Limited by measurement window and system noise
//! - **Requirements**: Functional PIT hardware and stable timing
//! - **Method**: Measure TSC delta over known PIT countdown period
//! - **Availability**: Universal (all x86 systems have PIT)
//!
//! ## Key Functions
//!
//! ### TSC Reading
//! * [`rdtsc`] - Read current TSC value with serialization fence
//!
//! ### Frequency Detection
//! * [`estimate_tsc_hz`] - Multi-method TSC frequency detection
//! * [`cpuid_leaf_15_tsc_hz`] - CPUID.15H crystal-based frequency
//! * [`cpuid_leaf_16_base_mhz_hz`] - CPUID.16H base frequency estimation
//! * [`pit_measure_tsc_hz`] - PIT-based measurement calibration
//!
//! ## PIT Calibration Details
//!
//! When CPUID methods fail, the module falls back to PIT-based measurement:
//!
//! ### Calibration Process
//! 1. **PIT Setup**: Configure Channel 0 in Mode 2 (rate generator)
//! 2. **Window Selection**: Use configurable measurement window (typically 50ms)
//! 3. **Synchronization**: Read TSC before and after PIT countdown
//! 4. **Calculation**: Convert TSC delta to frequency using known time window
//!
//! ### PIT Configuration
//! ```text
//! Channel 0: Rate generator mode (Mode 2)
//! Input Frequency: 1,193,182 Hz (standard PIT frequency)
//! Reload Value: Calculated for desired measurement window
//! Access Mode: Low byte, then high byte
//! ```
//!
//! ## Timing Accuracy Considerations
//!
//! ### Measurement Quality
//! * **Serialization**: `lfence` instruction ensures TSC read ordering
//! * **Interrupt Masking**: Caller should mask interrupts during calibration
//! * **Window Size**: Larger measurement windows improve accuracy
//! * **System Load**: Reduced system activity during measurement improves precision
//!
//! ### Error Sources
//! * **CPU Power Management**: Frequency scaling can affect measurements
//! * **Virtualization**: VMs may report inaccurate or unstable frequencies
//! * **Hardware Variations**: Manufacturing tolerances in crystal oscillators
//! * **Thermal Effects**: Temperature changes can affect oscillator frequency
//!
//! ## Usage Patterns
//!
//! ### Basic Frequency Detection
//! ```rust
//! let tsc_hz = unsafe { estimate_tsc_hz() };
//! println!("TSC frequency: {} Hz", tsc_hz);
//! ```
//!
//! ### High-Resolution Timing
//! ```rust
//! let start = rdtsc();
//! // ... timed operation ...
//! let end = rdtsc();
//! let cycles = end - start;
//! let nanoseconds = (cycles * 1_000_000_000) / tsc_hz;
//! ```
//!
//! ## Safety Considerations
//!
//! All TSC operations are marked unsafe due to:
//! * **Hardware Access**: Direct interaction with CPU counters and PIT hardware
//! * **I/O Port Usage**: PIT calibration requires port I/O operations
//! * **Timing Sensitivity**: Measurement accuracy depends on execution environment
//! * **Privilege Requirements**: Some operations require kernel-level access
//!
//! ## Architectural Notes
//!
//! ### TSC Properties (Modern CPUs)
//! * **Constant Rate**: TSC increments at fixed frequency regardless of CPU frequency
//! * **Non-Stop**: Continues incrementing during sleep states
//! * **Synchronized**: Same frequency across all cores in SMP systems
//! * **64-bit Range**: Extremely long rollover period (centuries at GHz frequencies)
//!
//! ### Compatibility
//! * **Intel**: Full support for all detection methods
//! * **AMD**: Partial CPUID support, PIT fallback available
//! * **Virtual Machines**: Variable support, often requires PIT calibration
//! * **Legacy Systems**: PIT calibration provides universal compatibility

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
