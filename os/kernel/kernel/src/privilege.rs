//! # Privilege Levels (x86-64)
//!
//! This module defines the **privilege level hierarchy** used by the CPU to
//! isolate protection domains (e.g. kernel vs. user mode).
//!
//! ## Overview
//!
//! x86-64 implements four hierarchical *rings* (`0–3`), of which only two are
//! normally used in long mode:
//!
//! - **Ring 0** — kernel / supervisor mode (highest privilege)
//! - **Ring 3** — user mode (least privilege)
//!
//! These levels appear in three distinct contexts that the CPU enforces rules
//! between:
//!
//! | Concept | Stored in | Purpose |
//! |----------|------------|----------|
//! | [`Ring`] | the current `CS` selector | the **current privilege level** (`CPL`) |
//! | [`Rpl`]  | the low 2 bits of a selector | the **requested privilege level** from the requester |
//! | [`Dpl`]  | bits 45–46 of a descriptor | the **descriptor privilege level** of the target |
//!
//! The CPU performs access checks such as:
//!
//! - **Data segment load:** `max(CPL, RPL) ≤ DPL`
//! - **Non-conforming code:** `CPL == DPL`
//! - **Conforming code:** `CPL ≤ DPL`
//! - **Stack segment (long mode):** `CPL == RPL == DPL`
//!
//! ## Structure
//!
//! This module re-exports:
//!
//! - [`Ring`] — enumeration of privilege rings (`Ring0` … `Ring3`).
//! - [`Rpl`] — requested privilege level plus helpers for selectors.
//! - [`Dpl`] — descriptor privilege level plus access-check predicates.
//! - [`KERNEL_RPL`]/[`USER_RPL`] — constants for common use.
//!
//! Together, these types provide a clear, type-safe way to reason about
//! privilege levels in descriptor and selector code, such as when building a
//! GDT, TSS, or when transitioning between user and kernel mode.
//!
//! ## Example
//!
//! ```rust
//! use arch_x86_64::privilege::{Ring, Rpl, Dpl, USER_RPL};
//!
//! let cpl = Ring::Ring0; // current level
//! let rpl = USER_RPL;    // selector carries Ring3
//! let dpl = Dpl::Ring3;  // descriptor is user-accessible
//!
//! assert!(dpl.permits_data_load(cpl, rpl));
//! ```
//!
//! ## References
//! - Intel® SDM, Vol 3A: *Protection; Descriptor Privilege Level (DPL)*
//! - AMD64 Architecture Programmer’s Manual, Vol 2: *System Programming*

#![allow(dead_code)]

mod dpl;
mod ring;
mod rpl;

pub use dpl::Dpl;
pub use ring::Ring;
pub use rpl::Rpl;
