//! # Kernel Memory Allocation and Virtual Memory Management
//!
//! This crate provides the core memory allocation infrastructure for the kernel,
//! implementing both physical frame allocation and virtual memory management
//! capabilities. It serves as the foundation for all memory operations in the
//! operating system, from initial bootstrap through runtime operation.
//!
//! ## Architecture Overview
//!
//! The memory management system is built on a three-layer architecture:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                Virtual Memory Manager (VMM)         │
//! │    • Page table manipulation                        │
//! │    • Virtual address space management               │
//! │    • User/kernel space separation                   │
//! └─────────────────┬───────────────────────────────────┘
//!                   │
//! ┌─────────────────▼───────────────────────────────────┐
//! │              Physical Mapper                        │
//! │    • Physical-to-virtual address translation        │
//! │    • HHDM (Higher Half Direct Mapping)              │
//! │    • Safe pointer conversion                        │
//! └─────────────────┬───────────────────────────────────┘
//!                   │
//! ┌─────────────────▼───────────────────────────────────┐
//! │           Physical Frame Allocator                  │
//! │    • 4KiB page frame management                     │
//! │    • Bitmap-based free/used tracking                │
//! │    • No-heap allocation strategy                    │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! ## Core Components
//!
//! ### Physical Frame Allocator ([`frame_alloc`])
//!
//! Manages the allocation and deallocation of 4KiB physical memory frames:
//! * **Bitmap Management**: Efficient tracking of free/used frames using bit arrays
//! * **No-Heap Design**: Self-contained implementation requiring no dynamic allocation
//! * **Fixed Region**: Manages a predefined region of physical memory (currently 512 MiB)
//! * **Early Boot Support**: Suitable for use before full memory management is available
//!
//! Key features:
//! - O(1) allocation when frames are available
//! - Simple bitmap-based tracking for reliability
//! - Configurable memory region boundaries
//! - Integration with kernel memory layout
//!
//! ### Physical Mapper ([`phys_mapper`])
//!
//! Provides safe conversion between physical addresses and virtual pointers:
//! * **HHDM Support**: Higher Half Direct Mapping for efficient address translation
//! * **Safe Abstractions**: Type-safe pointer conversions with lifetime management
//! * **Page Table Access**: Enables manipulation of physical page table structures
//! * **Cross-Platform**: Abstracts physical memory access patterns
//!
//! Key capabilities:
//! - Physical address to virtual pointer conversion
//! - Support for different mapping strategies
//! - Safe dereferencing of physical memory
//! - Integration with page table manipulation
//!
//! ### Virtual Memory Manager ([`vmm`])
//!
//! Coordinates virtual address space management and page table operations:
//! * **Address Space Management**: Separate user and kernel virtual address spaces
//! * **Page Table Manipulation**: Creation, modification, and destruction of mappings
//! * **Memory Protection**: Configurable page permissions (read, write, execute, user)
//! * **Anonymous Mapping**: On-demand allocation of virtual memory regions
//! * **Region Management**: Bulk operations for mapping and unmapping areas
//!
//! Advanced features:
//! - Guard page support for stack overflow detection
//! - Bulk mapping operations for performance
//! - TLB management and invalidation
//! - User/kernel space isolation
//!
//! ## Memory Layout Integration
//!
//! The crate integrates with the kernel's memory layout defined in `kernel-info`:
//!
//! ```text
//! Virtual Address Space Layout:
//! 0x0000_0000_0000_0000 ┌─────────────────────────────────┐
//!                       │        User Space               │
//!                       │  (Applications, libraries)      │
//! LAST_USERSPACE_ADDRESS├─────────────────────────────────┤
//!                       │        Guard Region             │
//! HHDM_BASE             ├─────────────────────────────────┤
//!                       │   Higher Half Direct Map        │
//!                       │  (Physical memory access)       │
//! KERNEL_BASE           ├─────────────────────────────────┤
//!                       │       Kernel Space              │
//!                       │  (Kernel code and data)         │
//! 0xFFFF_FFFF_FFFF_FFFF └─────────────────────────────────┘
//! ```
//!
//! ## Safety Model
//!
//! The memory management system employs multiple layers of safety:
//!
//! ### Type Safety
//! * **Address Types**: Distinct types for physical and virtual addresses
//! * **Page Alignment**: Compile-time guarantees for page-aligned operations
//! * **Lifetime Management**: Rust ownership prevents use-after-free errors
//!
//! ### Runtime Safety
//! * **Bounds Checking**: Validation of memory region boundaries
//! * **Permission Enforcement**: Hardware-backed memory protection
//! * **Guard Pages**: Overflow detection through unmapped regions
//! * **TLB Synchronization**: Proper cache invalidation on mapping changes
//!
//! ### Concurrency Safety
//! * **Atomic Operations**: Thread-safe allocation algorithms
//! * **Critical Sections**: Protection of shared data structures
//! * **Lock-Free Paths**: Performance optimization for common operations
//!
//! ## Usage Patterns
//!
//! ### Basic Physical Allocation
//! ```rust
//! use kernel_alloc::frame_alloc::BitmapFrameAlloc;
//! use kernel_vmem::PhysFrameAlloc;
//!
//! let mut allocator = BitmapFrameAlloc::new();
//! if let Some(frame) = allocator.alloc_4k() {
//!     // Use the physical frame
//!     allocator.free_4k(frame);
//! }
//! ```
//!
//! ### Virtual Memory Management
//! ```rust,no_run
//! use kernel_alloc::{phys_mapper::HhdmPhysMapper, vmm::Vmm};
//! use kernel_alloc::frame_alloc::BitmapFrameAlloc;
//!
//! let mapper = HhdmPhysMapper;
//! let mut allocator = BitmapFrameAlloc::new();
//! let mut vmm = unsafe { Vmm::from_current(&mapper, &mut allocator) };
//!
//! // Map virtual memory regions, manage page tables, etc.
//! ```
//!
//! ## Performance Characteristics
//!
//! * **Physical Allocation**: O(n) worst case, O(1) typical case
//! * **Virtual Mapping**: O(1) for single pages, O(n) for regions
//! * **Address Translation**: O(1) with HHDM
//! * **Memory Overhead**: ~1 bit per 4KiB frame for allocation tracking
//!
//! ## Integration Points
//!
//! This crate integrates with several other kernel components:
//! * **kernel-vmem**: Core virtual memory abstractions and types
//! * **kernel-info**: Memory layout constants and configuration
//! * **kernel-sync**: Synchronization primitives for thread safety
//!
//! The modular design enables testing, portability, and future enhancements
//! while maintaining clear separation of concerns between different memory
//! management responsibilities.

#![cfg_attr(not(any(test, doctest)), no_std)]

pub mod frame_alloc;
pub mod phys_mapper;
pub mod vmm;
