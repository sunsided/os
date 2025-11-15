# Developer Diary

## 2025-11-13

Syscall today. Refactored the crates a bit and upon wiring up the STAR/LSTAR registers
learned that I should rearrange the GDT to have Data Segments before Code Segments.
The swap was minimally invasive thankfully and things continued to work right after.

## 2025-11-12

There's some more work left to do with the INT80h parts. The fact that I currently
manually patch an explicitly known memory address of the user code table to make it
work is wild. I will have to update the VMM to either allocate disjoint tables
if any bit in the path is already marked system / not user-callable, but maybe
it makes even more sense to directly use dedicated VA bases for kernel and user code.
This doesn't solve the issue on its own (e.g. because of NX bits), but at least it
avoids user-callable bit issues and makes reasoning about memory ranges a bit easier.

Migrated to the `log` crate today. I'm hoping that this makes logging in the kernel
crates a bit easier, so that I can then focus on identifying the page issue
when not applying my debug patches to the user code.

The error from the days before was that I didn't consider the UEFI-side mapping.
Before bringup of the kernel, UEFI maps the trampoline code and stack into the CR3.
In my case, the currently executing binary was in the lower half at the beginning of RAM,
hence a low page table was used, and I used direct mapping on top. As a result, when
loading the userland code into its designated spot on the kernel side, the page
was already in use, marked not user, and ultimately partially marked not executable.
On top of that, the code to create the userland used the NX flag for the leafs, which
also contributed another reason for the page fault.

What I do now is this: After the kernel is fully set up (vmem, interrupts, etc.),
I set the lower half PML4 entries to zero, hence unmapping all UEFI-related code.
Since my own allocator never knew about these, I'm not leaking memory either. Now it works.

## 2025-11-09

Shot myself in the foot today: My spin mutex works, but it also silently blocks.
I was chasing ghosts when calling into userland when in reality I just attempted
to lock the mutex from within a mutex lock.

The `INT3` breakpoint handler now works, so it seems like usermode to kernel call works.
A bit later, the `INT80` syscall also worked. From what I can tell, returning values
via `rax` from syscall works, and the printing loop works as well. This is massive progress!

Trying my code in release build shortly after immediately got it stuck in `map_ist_stack`.
I got it fixed by increasing the release mode stack from 16 KiB to the 32 KiB I have
been using during development; this immediately unblocked it.

## 2025-11-08

Today's topic is allocating a proper kernel stack for the bootstrap processor, i.e. not
running off the `BOOT_STACK` linker section. I decided to rework the previous days' hacks
into a `PerCpu` structure even though I am not dealing with multiple CPUs right now;
collecting everything together made it a lot simpler to reason about what's needed (i.e.,
no `statics` all over the place), although I would be lying if I claimed to have grasped
the whole picture just yet. That said, right now I am able to trampoline onto a properly
allocated stack (once again), and am now dealing with wiring in the second-stage boostrapping.

There's now also an early page fault handler that at least avoids the triple fault every time.

The DS/ES/SS reload after setting the GDT haunted me today. The LAPIC timer interrupt would
get stuck, seemingly without reason, on `iretq`. In reality, it caused a general protection
fault which I could observe after installing the GP handler. It tried to execute into segment
`0x38`, which clearly doesn't exist, and not on the `KERNEL_CS` as expected. Turns out that
reloading the DS/ES/SS was good, but I never actually reloaded the CS (code segment) itself.
After adding that in the `init_gdt_and_tss` function, the timer seemed to work significantly
better.

The code needs a severe cleanup after today. Right now the APIC timer works and is calibrated
against the TSC (via serialized `rdtsc` and some busy looping). From there, APIC timer
start and divisors are computed, and now it ticks 1 kHz.

## 2025-11-04

Still wrapping my head around GDT, IDT, TSS, ...
I ran into some issue yesterday evening where the kernel wouldn't start up; took me until
today to figure out that I just failed a debug assertion and my panic handler simply idle looped.
I now added some proper text output to it to avoid this in the future.
The test to see whether the IDT was installed was wrong, and now I'm using the `sidt` instruction
to actually fetch the IDT record instead of reading the first byte and hoping for the best.
The INT 80h handler now installs, but I have still no way to properly test it just yet.

On Memory Segmentation, Dr. ChatGPT says:

> A memory segment is a region of linear (virtual) memory defined by a segment descriptor in
> the GDT (Global Descriptor Table) or LDT (Local Descriptor Table). Each descriptor provides:
>
> * a base address,
> * a limit (size),
> * and attributes (read/write, execute, privilege level, etc.).
>
> In 16- and 32-bit protected mode, logical addresses consist of a segment selector + offset.
> The CPU combines them like this:
>
> ```
> linear_address = segment.base + offset
> ```
>
> and checks that the offset ≤ limit.
>
> **Example (protected mode):**
>
> ```asm
> mov eax, [ds:0x1234]
> ```
>
> Here `ds` selects a descriptor (e.g., base = 0x40000000, limit = 0xFFFFF).
> So the CPU fetches from linear address `0x40001234`.
>
> Thus, segmentation was used to isolate code/data and implement per-task address spaces before paging became dominant.
>
> Long mode disables nearly all of this:
>
> * The base of all code/data segments is forced to 0.
> * The limit is ignored (treated as 0xFFFF_FFFF_FFFF_FFFF).
> * The only segmentation that still partly works:
>   * FS and GS register bases can still hold non-zero 64-bit values.
>     * These are used by OSes to implement thread-local storage or CPU-local data.
>   * Privilege levels (DPL bits) of code segments still matter.
>   * System segments (TSS, LDT descriptors) still exist.
>
> So in long mode, a “memory segment” is _still a logical concept_ defined by a descriptor,
> but the hardware effectively treats all normal segments as flat, except `FS` and `GS`.

On **Task State Segments**, Dr. ChatGPT says:

> Despite the name, the Task State Segment is not a memory segment for your code or data.
> It’s a special data structure the CPU consults for privileged operations,
> historically meant for hardware task switching (which long mode no longer uses).
>
> The TSS lives in memory, and the GDT (Global Descriptor Table) contains a system
> descriptor (type = 0x9 / 0xB) that points to it.
>
> | Field            | Purpose                                                                                                                                       |
> |------------------|-----------------------------------------------------------------------------------------------------------------------------------------------|
> | rsp0, rsp1, rsp2 | Kernel stacks for CPL = 0,1,2. When the CPU transitions from user mode (CPL = 3) to kernel mode (CPL = 0), it loads `RSP` from `rsp0`.        |
> | ist1..ist7       | Optional **Interrupt Stack Table** entries — alternate stacks for specific interrupts (e.g., double fault, NMI, #DF).                         |
> | iopb_offset      | Offset of the I/O permission bitmap — controls which I/O ports the task can access. Setting this offset ≥ sizeof(TSS) disables IOPB entirely. |
>
> The CPU references the TSS only:
>
> * on privilege transitions (to pick a safe stack), and
> * when delivering interrupts using IST entries.

And

> ```
> +-------------------+          +-------------------+
> | GDT (Descriptors) |----+---> | Code Segment (base=0)   --> flat 64-bit memory
> | - Kernel code     |    |
> | - Kernel data     |    |
> | - User code       |    |
> | - User data       |    |
> | - TSS descriptor  |----+---> | TSS64 structure in memory
> |                   |          |  rsp0, ist1..7, iopb_offset
> +-------------------+          +-------------------+
> ```

Before continuing wiring userland in, I'm thinking to refactor the half-state I have right now into
a per-CPU struct. Not that I expect SMP anytime soon, but it might just help myself understand better
which information I need to keep together to set up the CPU/kernel.

## 2025-11-02

Found [Task](https://taskfile.dev/) today and migrated from `Justfile` to `Taskfile.yaml`. It's
a bit of a paradigm shift but the added complexity made release builds significantly easier
(although I still not _entirely_ happy with it). It's good to see that the framebuffer pixel pokes
are not entirely slow under a proper build.

I also started with the initial userspace code logic yesterday. The idea is to use a CPIO "initramfs"
style filesystem. UEFI could load it into RAM and map it, then hand it over to the kernel. This way I
can get my first userspace code to load without having to implement a full-blown filesystem
driver. To that I decided I'd go with classic FAT16 first, read-only to begin with, but there's
still a _long_ way until I get there. The first idea is to mimic user code as a function in the
Kernel itself - doesn't matter where the code came from, after all - and then implement very basic
INT 80h style context switching. With an extremely basic initial syscall for poking into the QEMU
debug port and returning some numerical value I can try to set up a very first task. Preempting
would then be next.

## 2025-10-31

... and touching the Virtual Memory again, this time to add PAT (Page Attribute Table) bits
again. The idea was to enable write-combining for the framebuffer to squeeze out some performance,
and to do so, I decided to reimplement the individual page table entries as specifically typed structs
and unions, depending on their variant (useful, because leaves and entries can have different layouts,
especially across the layers). I added a unified view on top of that to manage bits logically rather
than by position. Not sure if in the end this made things faster or if the performance increase
in the framebuffer fill is just due to the optimization of the loop itself, but at least it
_feels_ faster ... or better.

## 2025-10-30

Rewriting the virtual memory to explicitly size-typed pages has been quite a journey; strangely,
everything worked out right away when firing up the emulator. I'm still not happy with the UEFI
side of things, especially around the ELF loader and the initial page table setup, but I'm more than
happy with the kernel-vmem libray now.

I also realized that the naive `alloc_range` is working now but definitely a candidate for more elaborate
implementation, and that deallocation, defragmentation and the likes are a game for themselves.
Future me: I'm sorry you have to deal with that again once the userland applications begin allocating
in a loop.

## 2025-10-28

Realized I had a misconception with the virtual memory virtual address / physical address mapping
today: We cannot just map any arbitrary VA onto any arbitrary PA. The PAs in the page table
always have their lower bits zeroed because they are page-aligned: The 4 KiB page has the lower
12 bits set to zero because the pages are 4 KiB aligned, and 4096 is `0b1000000000000`. By the same logic,
2 MiB and 1 GiB pages are zeroed out on the lower 21 and 30 bits respectively.
The actual physical address is determined by offset into the page, and that offset has to either be tracked
kernel-side or simply align with the physical offset. Since the kernel is the one deciding which addresses
to hand out, there shouldn't be any issue: It just picks any arbitrary virtual base, a feasible ("allocated")
physical base and then ensures that the VA offset (i.e., lower bits) coincides with the PA offset
(i.e., lower bits). The virtual memory mapper only sees pages and has no concept about the offsets whatsoever;
this is all done in "user code", which in this case is the kernel.

## 2025-10-26

I have started wrapping my head around the Virtual Memory Manager.

I also learned about the `-monitor stdio` QEMU option as well as the `-debugcon file:debug.log`
variant.

I now added `-s` option (i.e., `-gdb tcp::1234`) to the QEMU command line, allowing for remote debugging
in RustRover. The `add-symbol-file qemu/esp/EFI/Boot/kernel.elf` command needs to be executed when
in Kernel mode (i.e., left UEFI); this can be done in the remote debug run configuration. I'm curious
to see how this works when running this in a higher-half kernel.

Since I need the information about the kernel's virtual memory address, as well as its physical location
in both the UEFI (for the ELF loader) and in the linker script, I decided to pull it into the `kernel-info`
crate which I now reuse in the kernel's `build.rs`. The linker script now uses `DEFINED(KERNEL_BASE)`
and `DEFINED(PHYS_LOAD)` to allow external configuration.

Tracing with QEMU got massively easier now with the `qemu_trace!` macro. I didn't think implementing
that would be so easy.

I'm not dealing with a handover into the higher-half, as the CPU hangs on the `CR3` instruction. Right now
I am only identity mapping the first 2 MiB (and then physmapping some more gigs on the high half).
The problem appears to be that my UEFI loader is not running in the first 2 MiB however, as is shown
by the trace output:

```plain
UEFI Loader reporting to QEMU
Exiting boot services ...
Boot services exited, we're now flying by instruments.
Boot Info in UEFI Loader:
   Kernel = 00ffffffff80100b5c (@17592186042369 MiB)
   BI ptr = 00000000000e76dba0 (@231 MiB)
       VA = 00000000000e76dba0 (@231 MiB)
 MMAP ptr = 00000000000e785020 (@231 MiB), len = 7248, desc size = 48, desc version = 1, rsdp addr = 259514388
   FB ptr = 000000000080000000 (@2048 MiB), size = 4096000, width = 1280, height = 800, stride = 1280, format = BGR
Enabling supervisor write protection ...
Setting EFER.NXE ...
Enabling global pages ...
Loading CR3 with the Page Table Root ...
```

It's clear that the currently executing code is at 231 MiB and therefore not mapped, which then leads
to a page fault. Adding `-no-reboot -no-shutdown -d cpu_reset` to the QEMU command line makes
this a bit more obvious:

```plain
(qemu) CPU Reset (CPU 0)
EAX=00000000 EBX=00000000 ECX=00000000 EDX=00060fb1
ESI=00000000 EDI=00000000 EBP=00000000 ESP=00000000
EIP=0000fff0 EFL=00000002 [-------] CPL=0 II=0 A20=1 SMM=0 HLT=0
ES =0000 00000000 0000ffff 00009300
CS =f000 ffff0000 0000ffff 00009b00
SS =0000 00000000 0000ffff 00009300
DS =0000 00000000 0000ffff 00009300
FS =0000 00000000 0000ffff 00009300
GS =0000 00000000 0000ffff 00009300
LDT=0000 00000000 0000ffff 00008200
TR =0000 00000000 0000ffff 00008b00
GDT=     00000000 0000ffff
IDT=     00000000 0000ffff
CR0=60000010 CR2=00000000 CR3=00000000 CR4=00000000
DR0=0000000000000000 DR1=0000000000000000 DR2=0000000000000000 DR3=0000000000000000
DR6=00000000ffff0ff0 DR7=0000000000000400
CCS=00000000 CCD=00000000 CCO=DYNAMIC
EFER=0000000000000000
FCW=037f FSW=0000 [ST=0] FTW=00 MXCSR=00001f80
FPR0=0000000000000000 0000 FPR1=0000000000000000 0000
FPR2=0000000000000000 0000 FPR3=0000000000000000 0000
FPR4=0000000000000000 0000 FPR5=0000000000000000 0000
FPR6=0000000000000000 0000 FPR7=0000000000000000 0000
XMM00=0000000000000000 0000000000000000 XMM01=0000000000000000 0000000000000000
XMM02=0000000000000000 0000000000000000 XMM03=0000000000000000 0000000000000000
XMM04=0000000000000000 0000000000000000 XMM05=0000000000000000 0000000000000000
XMM06=0000000000000000 0000000000000000 XMM07=0000000000000000 0000000000000000
Triple fault
```

A bunch of issues caused that. At the core, again, seems to have been the `win64` calling convention
requiring extra stack space. After successfully setting that up, the kernel still segfaulted
because it was now trying to access the framebuffer which, of course, was not mapped. While entirely
on purpose, this sent me searching for bugs in the UEFI loaded when in reality the error came from elsewhere.
Had I looked closer in the `debug.log`, I could have noticed ...
Anyway, to set up the memory map in the kernel it needed to allocate, so now I needed to cook up an allocator as well.

After implementing a basic virtual memory manager (VMM) on the Kernel to map the framebuffer,
I reverted the UEFI/kernel trampoline to `sysv64` ABI.

## 2025-10-25

Adding a serial output to the QEMU emulator turned out to be extremely helpful in finding out
what was going on when my UEFI loader would simply spin indefinitely and then get killed by QEMU
after some 60 seconds. It turned out that the calling convention was problematic: having my
Kernel `_start_kernel` as `extern "C"` (assuming the first argument, the `KernelBootInfo` struct
would be passed in `rdi`) was a bad idea: The UEFI code is PE/COFF and used `win64` calling conventions,
setting up the argument in `rcx` instead. Changing calling conventions a bit and setting up a naked
jump pad in `_start_kernel` fixed the problem.

I then added a bit of RSDP/XSDP, i.e., ACPI 1.0 and 2.0 parsing, only to later on learn that QEMU
currently does not even support ACPI 2.0.
