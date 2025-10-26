//! # Tiny Free-List Kernel Allocator
//!
//! A minimal, `no_std`-friendly global allocator intended for early boot / hobby
//! kernels. The implementation manages a single statically reserved heap using a
//! **singly linked free-list** with headers embedded in free blocks.
//!
//! ## Design outline
//! - **Storage**: a single `.bss`-backed byte array (`HEAP`) is treated as the heap.
//! - **Free-list nodes**: each free block starts with a [`ListNode`](free_list::ListNode) header
//!   followed by `size` bytes of payload. The header is *part of the free block*.
//! - **Allocation strategy**: first-fit with **alignment**. Blocks are split into
//!   up to two remainders (head/tail). The chosen block’s header is removed from
//!   the free list, and the allocation returns the aligned payload pointer.
//! - **Deallocation**: the allocator expects the original `Layout` (size and
//!   alignment). It recreates a free block by placing a header immediately before
//!   the returned pointer and reinserts it into the list in address order.
//! - **Coalescing**: adjacent free blocks are merged on insert to combat
//!   fragmentation.
//! - **Synchronization**: a tiny [`SpinLock`](kernel_sync::spin_lock::SpinLock) guards all allocator operations.
//!
//! ## Constraints & caveats
//! - Designed for **uniprocessor** or very early boot. For SMP, either keep
//!   allocations short and rare or replace [`SpinLock`](kernel_sync::spin_lock::SpinLock) with a stronger primitive.
//! - Interrupts are **not** masked by the lock; if you allocate in interrupt
//!   context, ensure you won’t deadlock.
//! - `dealloc` must receive the same `Layout` used for `alloc` (or a layout with
//!   the same `size`), as mandated by `GlobalAlloc`.
//! - This allocator does **not** grow; its capacity is fixed by `HEAP_SIZE`.
//!
//! ## When to use
//! - Early boot, kernel bring-up, test kernels, QEMU experiments.
//! - Not intended as a production-grade, scalable SMP allocator.
//!
//! ## Related items
//! - [`KernelAllocator`](kernel_allocator::KernelAllocator) implements `GlobalAlloc` and is installed via
//!   [`GLOBAL_ALLOCATOR`](kernel_allocator::GLOBAL_ALLOCATOR).
//! - The heap is lazily initialized on the first allocation through [`ensure_init`](static_heap::ensure_init).
//!
//! ## Safety audit points
//! - All interior `unsafe` is confined to well-documented sections.
//! - Free-list pointer manipulation is performed only while holding the lock.
//!
//! This file deliberately uses a small amount of `unsafe` to manage raw memory
//! and uphold `GlobalAlloc`’s contract in a `no_std` environment.

#![cfg_attr(not(test), no_std)]
#![allow(unsafe_code)]

mod free_list;
mod kernel_allocator;
mod static_heap;
