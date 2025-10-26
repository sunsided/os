# Developer Diary

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
