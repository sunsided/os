//! # x86-64 Higher-Half Kernel
//!
//! A bare-metal x86-64 kernel implementing a modern operating system foundation with
//! virtual memory management, interrupt handling, and basic userland support.
//!
//! ## Overview
//!
//! This kernel is designed as a higher-half kernel loaded at `0xffffffff80000000` with
//! physical load address at 1 MiB. It boots from UEFI and implements essential OS
//! services including:
//!
//! * **Memory Management**: Virtual memory manager (VMM) with page tables, physical
//!   frame allocator, and HHDM (Higher Half Direct Mapping) for kernel space
//! * **Interrupt Handling**: IDT setup with handlers for exceptions, page faults,
//!   timer interrupts, and system calls
//! * **APIC Integration**: Local APIC timer calibrated against TSC (Time Stamp Counter)
//! * **Process Management**: Basic userland task creation and privilege level switching
//! * **Graphics**: UEFI GOP framebuffer support with basic rendering capabilities
//!
//! ## Boot Process
//!
//! 1. **UEFI Entry**: [`_start_kernel`](init::_start_kernel) receives control from UEFI loader
//! 2. **Stack Setup**: Establishes boot stack and switches to kernel stack
//! 3. **Memory Init**: Initializes physical allocator and virtual memory manager
//! 4. **CPU Setup**: Configures GDT, TSS, IDT, and per-CPU data structures
//! 5. **APIC Config**: Sets up Local APIC and calibrates timer frequency
//! 6. **Framebuffer**: Maps UEFI GOP framebuffer into kernel virtual space
//! 7. **Main Loop**: Enters [`kernel_main`] for ongoing operation
//!
//! ## Architecture
//!
//! The kernel follows a monolithic design with modular components:
//!
//! * `alloc`: Memory allocation and virtual memory management
//! * `interrupts`: Exception and interrupt handling subsystem
//! * `apic`: Advanced Programmable Interrupt Controller support
//! * `gdt`/`tss`: Global Descriptor Table and Task State Segment
//! * `userland`: User mode task creation and privilege switching
//! * `framebuffer`: Graphics and display management
//!
//! ## Main Loop Behavior
//!
//! The kernel's main loop demonstrates a breathing LED effect by:
//! - Measuring timer frequency against TSC
//! - Calculating sinusoidal brightness values over 2-second periods
//! - Updating framebuffer with computed brightness
//! - Transitioning to userland after 2 seconds
//!
//! ## Safety
//!
//! This kernel operates in a `no_std` environment with extensive use of `unsafe` code
//! for low-level hardware interaction, memory management, and privilege level changes.
//! All unsafe operations are carefully isolated and documented.

#![cfg_attr(not(any(test, doctest)), no_std)]
#![no_main]
#![allow(unsafe_code)]

mod alloc;
mod apic;
mod cpuid;
mod framebuffer;
mod gdt;
mod idt;
mod init;
mod interrupts;
mod msr;
mod panik;
mod per_cpu;
mod ports;
mod privilege;
mod task;
mod tracing;
mod tsc;
mod tss;
mod userland;
mod userland_demo;

use crate::alloc::with_kernel_vmm;
use crate::framebuffer::fill_solid;
use crate::per_cpu::PerCpu;
use crate::tsc::{estimate_tsc_hz, rdtsc};
use crate::userland::boot_single_user_task;
use core::f32::consts::{PI, TAU};
use core::hint::spin_loop;
use core::sync::atomic::{AtomicU64, Ordering};
use kernel_info::boot::FramebufferInfo;
use log::info;

/// Main kernel loop, running with all memory (including framebuffer) properly mapped.
///
/// # Entry point
/// UEFI enters the kernel in [`_start_kernel`](init::_start_kernel), from where
/// we initialize the boot stack, set up memory management and then jump here.
///
/// # Memory Safety
/// At this point, the kernel operates with virtual addresses set up by the VMM, and
/// the framebuffer is accessible at its mapped virtual address. All further kernel
/// code should use these mapped addresses, not physical ones.
///
/// # Arguments
/// * `fb_virt` - [`FramebufferInfo`] with a valid, mapped virtual address.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn kernel_main(fb_virt: &FramebufferInfo) -> ! {
    info!("Kernel doing kernel things now ...");

    let cpu = unsafe { PerCpu::current() };
    let start = cpu.ticks.load(Ordering::Acquire);
    let mut prev = 0;

    if TIMER_HZ.load(Ordering::Acquire) == 0 {
        let hz = unsafe { measure_timer_hz(cpu) };
        TIMER_HZ.store(hz.max(1), Ordering::Release);
        info!("Observed timer rate ≈ {hz} Hz");
    }

    loop {
        let ticks = cpu.ticks.load(Ordering::Acquire);
        let hz = TIMER_HZ.load(Ordering::Acquire);

        // Phase from integer modulo: 2-second period
        let period_ticks = 2 * hz; // 2 s
        let t_in_period = (ticks - start) % period_ticks;

        // Map to [-π, π]
        let tau = TAU;
        let pi = PI;
        let phase_0_2pi = (t_in_period as f32) * (tau / period_ticks as f32);
        let x = if phase_0_2pi > pi {
            phase_0_2pi - tau
        } else {
            phase_0_2pi
        };

        let s = fast_sin(x); // ~[-1, +1]

        // Scale to [0, 255]
        let brightness = ((s + 1.0) * 0.5 * 255.0) as u8;

        // Optional: second counter (kept from your code)
        let seconds = ((ticks - start) as f32) / hz as f32;
        if (seconds as u32) > prev {
            prev = seconds as u32;
            info!("Kernel cycle: {prev} s");
        }

        unsafe { fill_solid(fb_virt, 72, 0, brightness) };
        spin_loop();

        if prev == 2 {
            info!("Jumping into userland code - will not refresh screen anymore");
            with_kernel_vmm(|vmm| {
                boot_single_user_task(vmm);
            });
        }
    }
}

#[inline]
fn fast_sin(x: f32) -> f32 {
    // x must be in [-π, π]
    const B: f32 = 4.0 / core::f32::consts::PI;
    const C: f32 = -4.0 / (core::f32::consts::PI * core::f32::consts::PI);
    // First-order: triangle-like sine
    let y = B * x + C * x * x.abs();
    // Second-order correction for curvature
    // (Keeps result in [-1, 1], continuous and smooth enough for fades)
    y * (0.775 + 0.225 * (y.abs() - 1.0))
}

pub static TIMER_HZ: AtomicU64 = AtomicU64::new(0);

unsafe fn measure_timer_hz(cpu: &PerCpu) -> u64 {
    let tsc_hz = unsafe { estimate_tsc_hz() };
    let window_tsc = tsc_hz / 10; // ~100 ms

    let t0_tsc = rdtsc();
    let t0_ticks = cpu.ticks.load(Ordering::Acquire);

    // Busy-wait until ~100 ms have elapsed.
    loop {
        if rdtsc().wrapping_sub(t0_tsc) >= window_tsc {
            break;
        }
        spin_loop();
    }

    let t1_ticks = cpu.ticks.load(Ordering::Acquire);
    let dt_ticks = t1_ticks.saturating_sub(t0_ticks);

    // Scale up to per-second
    u128::from(dt_ticks)
        .saturating_mul(10) // 1 / 0.1s
        .try_into()
        .unwrap_or(u64::MAX)
}
