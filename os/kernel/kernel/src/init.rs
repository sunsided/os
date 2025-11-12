use crate::idt::{idt_update_in_place, init_idt_once};
use crate::interrupts::syscall::SyscallInterrupt;
use crate::interrupts::{Idt, Ist};
use crate::tracing::trace_boot_info;
use crate::{gdt, interrupts, kernel_main};
use kernel_info::boot::{FramebufferInfo, KernelBootInfo};
use kernel_qemu::QemuLogger;
use log::{LevelFilter, info};

use crate::alloc::{
    FlushTlb, init_kernel_vmm, init_physical_memory_allocator_once, try_with_kernel_vmm,
};
use crate::apic::{init_lapic_and_set_cpu_id, start_lapic_timer};
use crate::cpuid::CpuidRanges;
use crate::framebuffer::VGA_LIKE_OFFSET;
use crate::interrupts::bp::BreakpointInterrupt;
use crate::interrupts::df::DfInterrupt;
use crate::interrupts::gp::GeneralProtectionFaultInterrupt;
use crate::interrupts::page_fault::PageFaultInterrupt;
use crate::interrupts::spurious::SpuriousInterrupt;
use crate::interrupts::ss::SegmentFaultInterrupt;
use crate::interrupts::timer::TimerInterrupt;
use crate::msr::init_gs_bases;
use crate::per_cpu::PerCpu;
use crate::per_cpu::ist_stacks::{IST1_SIZE, ist_slot_for_cpu};
use crate::per_cpu::kernel_stacks::kstack_slot_for_cpu;
use crate::per_cpu::stack::{CpuStack, map_ist_stack, map_kernel_stack};
use crate::tsc::estimate_tsc_hz;
use kernel_alloc::phys_mapper::HhdmPhysMapper;
use kernel_alloc::vmm::AllocationTarget;
use kernel_info::memory::{HHDM_BASE, KERNEL_STACK_SIZE};
use kernel_sync::irq::sti_enable_interrupts;
use kernel_vmem::VirtualMemoryPageBits;
use kernel_vmem::addresses::{PhysicalAddress, VirtualAddress};

/// Earliest boot stack size. This stack is used only when handing over from UEFI
/// to the Kernel, and then immediately changed for a properly allocated stack.
pub const BOOT_STACK_SIZE: usize = 64 * 1024;

/// A byte buffer with a **guaranteed 16-byte alignment**.
///
/// This thin newtype is used to back stacks (or other raw buffers) that must
/// satisfy the x86-64 SysV ABI alignment rules. In particular, we want the
/// stack pointer (`RSP`) to be 16-byte aligned at call boundaries and when
/// entering assembly routines that assume vector-friendly alignment.
///
/// - The inner array has length `N`.
/// - The wrapper enforces `align(16)`, regardless of the platform’s default.
///
/// # Why 16 bytes?
/// Many instructions and calling conventions (e.g., SSE/AVX spills, ABI rules)
/// expect 16-byte alignment. Some exception/interrupt paths also benefit from a
/// predictable alignment, so we keep our boot stack aligned from the start.
///
/// # Examples
/// ```
/// # use core::mem::{align_of, size_of};
/// # struct Aligned<const N: usize>([u8; N]);
/// # #[repr(align(16))] struct A<const N: usize>([u8; N]);
/// # let _ = (size_of::<A<4096>>(), align_of::<A<4096>>());
/// // 4096 bytes with 16-byte alignment
/// // let buf = Aligned::<4096>([0; 4096]);
/// ```
#[repr(align(16))]
struct Aligned16<const N: usize>([u8; N]);

/// Early **boot stack** memory placed in a dedicated BSS section,
/// used by the boostrap CPU (BSP).
///
/// This stack is used before the normal kernel allocator and task stacks exist
/// (e.g., right after firmware/bootloader hand-off). It is:
///
/// - **Static** and zero-initialized (lives in `.bss.boot`).
/// - **16-byte aligned** via the `Aligned` wrapper.
/// - **`static mut`** because low-level setup code manipulates it via raw
///   pointers. Access must be controlled to avoid data races.
///
/// Attributes:
/// - `#[unsafe(link_section = ".bss.boot")]` ensures the symbol is emitted into
///   a dedicated section (useful for linker scripts, paging attributes, or for
///   keeping boot-only data together).
/// - `#[unsafe(no_mangle)]` gives the symbol a stable name so hand-written
///   assembly (e.g., early entry stubs) can refer to it.
///
/// # Safety
/// - Only one CPU/core should use this stack at a time. Treat it as **single-
///   owner**, early-boot scratch space.
/// - Do not use once per-CPU or task stacks are set up.
/// - Writing/reading it requires `unsafe` because it’s `static mut`.
#[unsafe(link_section = ".bss.boot")]
#[unsafe(no_mangle)]
static mut BOOT_STACK: Aligned16<BOOT_STACK_SIZE> = Aligned16([0; BOOT_STACK_SIZE]);

const _: () = assert!(
    BOOT_STACK_SIZE.is_multiple_of(16),
    "BOOT_STACK_SIZE should be 16-byte aligned"
);

/// The kernel entry point
///
/// # UEFI Interaction
/// The UEFI loader will jump here after `ExitBootServices`.
///
/// # ABI
/// The ABI is defined as `sysv64` (Rust's `extern "C"`), so the kernel is called
/// with the `boot_info` pointer in `RDI` (System V AMD64 ABI, as on Linux/x86_64).
///
/// # Naked function & Stack
/// This is a naked function in order to set up the stack ourselves. Without
/// the `naked` attribute (and the [`naked_asm`](core::arch::naked_asm) instruction), Rust
/// compiler would apply its own assumptions based on the C ABI and would attempt to
/// unwind the stack on the call into [`kernel_entry_on_boot_stack`]. Since we're clearing out the stack
/// here, this would cause UB.
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start_kernel(_boot_info: *const KernelBootInfo) {
    core::arch::naked_asm!(
        "cli",
        // save RDI (boot_info per SysV64)
        "mov r12, rdi",
        // Build our own kernel stack and establish a valid call frame for kernel_entry
        "lea rax, [rip + {stack_sym}]",
        "add rax, {stack_size}",
        // Align down to 16
        "and rax, -16",
        // Set RSP to the prepared value
        "mov rsp, rax",
        // Emulate a CALL by pushing a dummy return address (so RSP % 16 == 8 at entry)
        "push 0",
        "xor rbp, rbp",
        // Restore boot_info into the expected arg register (SysV/C ABI)
        "mov rdi, r12",
        // Jump to Rust entry and never return
        "jmp {rust_entry}",
        stack_sym = sym BOOT_STACK,
        stack_size = const BOOT_STACK_SIZE,
        rust_entry = sym kernel_entry_on_boot_stack,
    );
}

/// Kernel entry running on the boot stack ([`BOOT_STACK`]).
///
/// # Notes
/// * `no_mangle` is used so that [`_start_kernel`] can jump to it by name.
/// * It uses C ABI to have a defined convention when calling in from ASM.
/// * The [`_start_kernel`] function keeps `boot_info` in `RDI`, matching C ABI expectations.
#[unsafe(no_mangle)]
pub extern "C" fn kernel_entry_on_boot_stack(boot_info: *const KernelBootInfo) -> ! {
    let logger = QemuLogger::new(LevelFilter::Debug);
    logger.init().expect("logger init");

    info!("Kernel reporting to QEMU! Initializing bootstrap processor now.");
    let info = unsafe { CpuidRanges::read() };
    info!("Running on {}", info.vendor.as_str());

    let bi = unsafe { &*boot_info };
    trace_boot_info(bi);

    info!("Initializing Virtual Memory Manager ...");
    initialize_memory_management();

    info!("Initializing Kernel stack ...");
    let kstack_top = initialize_kernel_stack();

    // Switch to the new stack (align already handled in map_kernel_stack)
    info!("Switching to boostrap processor kernel stack ...");
    unsafe {
        stage_one_switch_to_stack_and_enter(
            kstack_top,
            stage_two_init_bootstrap_processor,
            boot_info,
        );
    }
}

fn initialize_memory_management() {
    unsafe {
        // Initialize the bitmap allocator on the heap.
        // TODO: Restrict allocator to actual available RAM size.
        let alloc = init_physical_memory_allocator_once();
        info!(
            "Supporting {} MiB of physical RAM",
            alloc.manageable_size() / 1024 / 1024
        );

        // Initialize the VMM with the allocator.
        init_kernel_vmm(HhdmPhysMapper, alloc);
    }
}

fn initialize_kernel_stack() -> KernelStackTop {
    let kstack_cpu_slot = kstack_slot_for_cpu(0);
    info!("Designated CPU-specific stack base at {kstack_cpu_slot}.");
    info!("Allocating bootstrap processor kernel stack ...");
    let CpuStack {
        base: _base,
        top: kstack_top,
        len: _len,
    } = try_with_kernel_vmm(FlushTlb::OnSuccess, |vmm| {
        map_kernel_stack(vmm, kstack_cpu_slot, KERNEL_STACK_SIZE as u64)
    })
    .expect("map per-CPU kernel stack");

    info!("Probing new kernel stack at {kstack_top} ...");
    let probe = (kstack_top.as_u64() - 8) as *mut u64;
    unsafe {
        core::ptr::write_volatile(probe, 0xDEAD_BEEF_DEAD_BEEFu64);
        let _ = core::ptr::read_volatile(probe);
    }

    kstack_top
}

/// Naked jump pad: set RSP and jump; no Rust frame, no locals.
///
/// # Safety
/// This is a **naked jump pad** that switches the active stack pointer (`RSP`)
/// to a newly-mapped kernel stack and immediately jumps into another function.
/// It performs no prologue or epilogue and must never return.
///
/// ## Calling convention
/// This function uses the System V AMD64 (Unix x86-64) C ABI.
/// The caller must pass its arguments exactly as follows:
///
/// | Argument | Purpose                  | Register |
/// |-----------|--------------------------|-----------|
/// | `top`     | New stack top (RSP target) | `RDI` |
/// | `entry`   | Function pointer to jump to | `RSI` |
/// | `boot_info` | Pointer forwarded to `entry` | `RDX` |
///
/// The caller places these values in the given registers before the jump.
/// No arguments are passed on the stack.
///
/// ## Operation
/// ```text
/// mov rsp, rdi      ; switch to new stack
/// push 0            ; ensure RSP%16 == 8 for next call per SysV ABI
/// mov  rdi, rdx     ; move boot_info into first argument register
/// jmp  rsi          ; tail-jump to entry (never returns)
/// ```
///
/// The `push 0` maintains correct 16-byte stack alignment expected at function
/// entry. The trampoline then tail-jumps to `entry`, so control never returns
/// to this function.
///
/// ## Notes
/// * Do **not** mark this with `nostack`; it *does* modify `RSP`.
/// * The red-zone must be disabled for the kernel target (otherwise writes
///   below `RSP` may corrupt memory).
/// * Interrupts should be disabled while switching stacks.
/// * Because this function is `#[naked]`, you **must not** use any Rust locals
///   or reference its parameters in Rust code — only inline assembly may appear
///   in the body.
///
/// ## Invariants
/// * The target stack (`top`) must be fully mapped and writable.
/// * `entry` must be a valid, non-returning function adhering to the same ABI.
/// * On success, control never returns; on failure, behavior is undefined.
#[unsafe(naked)]
unsafe extern "C" fn stage_one_switch_to_stack_and_enter(
    _top: VirtualAddress,
    _entry: extern "C" fn(*const KernelBootInfo, VirtualAddress) -> !,
    _boot_info: *const KernelBootInfo,
) -> ! {
    core::arch::naked_asm!(
        // rdi=top, rsi=entry, rdx=boot_info
        "mov r11, rsi", // preserve 'entry' into r11 (caller-saved scratch)
        "mov rsp, rdi", // set stack = top (rdi still holds top)
        "push 0",       // align RSP so next call sees RSP%16 == 8
        "mov rsi, rdi", // rsi = top (second argument for callee)
        "mov rdi, rdx", // rdi = boot_info (first argument for callee)
        "jmp r11",      // tail jump to entry; never return
    )
}

/// Per-CPU configuration (GDT, IST, stack, ...)
///
/// Currently only one static configuration exists due to early single-core development.
/// Later on this might be changed into a vector. See [`slot_for_cpu`] for calculating
/// per-CPU stack offsets.
static mut PER_CPU0: PerCpu = PerCpu::new();

extern "C" fn stage_two_init_bootstrap_processor(
    boot_info: *const KernelBootInfo,
    kstack_top: KernelStackTop,
) -> ! {
    info!("Trampolined onto the kernel stack. Observing kernel stack top at {kstack_top}.");
    let bi = unsafe { &*boot_info };
    trace_boot_info(bi);

    info!("Allocating IST1 stack ..");
    let ist1_top = allocate_ist1_stack();

    // Initialize per-CPU configuration
    let cpu = initialize_percpu_config_for_bsp(kstack_top, ist1_top);

    info!("Initializing GDT and TSS ...");
    gdt::init_gdt_and_tss(cpu, kstack_top, ist1_top);

    // Point GS.base to &PerCpu for fast access
    unsafe {
        init_gs_bases(cpu);
    }

    info!("Remapping UEFI GOP framebuffer ...");
    let fb_virt = remap_framebuffer_memory(bi);

    // Initialize the IDT once.
    info!("Initializing IDT ...");

    unsafe {
        init_idt_once(Idt::new());
    }

    // Update the IDT. Enters a critical section and (re-)enables interrupts
    // when the function returns.
    info!("Installing interrupt handlers ...");
    idt_update_in_place(|idt| {
        idt.init_df_gate_ist(interrupts::df::double_fault_handler, Ist::Ist1); // TODO: Use a different IST from PF
        idt.init_breakpoint_gate(interrupts::bp::bp_handler);
        idt.init_syscall_gate();
        idt.init_ss_fault_gate(interrupts::ss::ss_fault_handler);
        idt.init_gp_fault_gate(interrupts::gp::gp_fault_handler);
        idt.init_page_fault_gate_ist(interrupts::page_fault::page_fault_handler, Ist::Ist1);
        idt.init_timer_gate(interrupts::timer::lapic_timer_handler);
        idt.init_spurious_interrupt_gate();
    });

    info!("Estimating TSC frequency ...");
    let tsc_hz = unsafe { estimate_tsc_hz() };
    trace_tsc_frequency(tsc_hz);

    // Init LAPIC, store LAPIC ID into per-CPU struct, then arm timer.
    init_lapic_and_set_cpu_id(cpu);
    start_lapic_timer(tsc_hz);

    info!("Enabling interrupts ...");
    sti_enable_interrupts();

    info!("Kernel early init is done, jumping into kernel main loop ...");
    kernel_main(&fb_virt)
}

type Ist1StackTop = VirtualAddress;
type KernelStackTop = VirtualAddress;

fn allocate_ist1_stack() -> Ist1StackTop {
    let (ist1_base, ist1_top) = try_with_kernel_vmm(FlushTlb::OnSuccess, |vmm| {
        let slot = ist_slot_for_cpu(0, Ist::Ist1);
        map_ist_stack(vmm, slot, IST1_SIZE)
    })
    .expect("map IST1");
    info!("IST1 mapped: base={ist1_base}, top={ist1_top}");
    ist1_top
}

fn initialize_percpu_config_for_bsp(
    kstack_top: KernelStackTop,
    ist1_top: Ist1StackTop,
) -> &'static mut PerCpu {
    #[allow(static_mut_refs)]
    let p = unsafe { &mut PER_CPU0 };
    p.cpu_id = 0;
    p.apic_id = 0; // will be set below by the APIC initialization.
    p.kstack_top = kstack_top;
    if let Some(idx) = Ist::Ist1.tss_index() {
        p.ist_stacks[idx] = ist1_top;
    }
    p
}

#[allow(clippy::cast_precision_loss)]
fn trace_tsc_frequency(tsc_hz: u64) {
    info!(
        "TSC frequency = {tsc_hz} Hz ({ghz:0.2} GHz)",
        ghz = (tsc_hz as f32) / 1000.0 / 1000.0 / 1000.0
    );
}

/// Remaps the boot framebuffer memory into the kernel's virtual address space.
///
/// UEFI provides the physical address of the framebuffer in the boot info, but does not
/// include it in the memory mapping table. This means the kernel must manually map the
/// framebuffer into its own virtual address space to access it. This function sets up the
/// necessary mapping so the framebuffer can be used by the kernel.
fn remap_framebuffer_memory(bi: &KernelBootInfo) -> FramebufferInfo {
    // Map framebuffer
    let fb_pa = PhysicalAddress::new(bi.fb.framebuffer_ptr);
    let fb_len = bi.fb.framebuffer_size;
    let va_base = VirtualAddress::new(HHDM_BASE) + VGA_LIKE_OFFSET;
    let fb_flags = VirtualMemoryPageBits::default()
        .with_writable(true)
        .with_write_combining()
        .with_global(true)
        .with_no_execute(true);

    try_with_kernel_vmm(FlushTlb::OnSuccess, |vmm| {
        vmm.map_region(
            AllocationTarget::Kernel,
            va_base,
            fb_pa,
            fb_len,
            fb_flags,
            fb_flags,
        )
    })
    .expect("Framebuffer mapping failed");

    // Return updated FramebufferInfo with new virtual address
    let mut fb_virt = bi.fb.clone();
    fb_virt.framebuffer_ptr = (va_base + (fb_pa.as_u64() & 0xFFF)).as_u64(); // preserve offset within page
    info!("Remapped frame buffer to {va_base}");
    fb_virt
}
