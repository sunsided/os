/* kernel.ld — higher-half kernel, loaded at 1 MiB */
OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start_kernel)

/* --- Tunables (overrideable via --defsym) --- */
KBASE = DEFINED(KERNEL_BASE) ? KERNEL_BASE : 0xffffffff80000000;  /* VMA base */
PLOAD = DEFINED(PHYS_LOAD)   ? PHYS_LOAD   : 0x00100000;          /* LMA base (1 MiB) */

/* Sanity checks (optional but handy) */
ASSERT((KBASE & ((1 << 21) - 1)) == 0, "KERNEL_BASE must be 2 MiB aligned");
ASSERT((PLOAD & 0xFFF) == 0, "PHYS_LOAD must be 4 KiB aligned");

PHDRS {
  text PT_LOAD FLAGS(5);   /* R+X */
  data PT_LOAD FLAGS(6);   /* R+W */
}

SECTIONS
{
  /* --- All section VMAs live in higher half --- */
  . = KBASE + PLOAD;

  /* Text (VMA = KBASE+..., LMA = PLOAD+offset) */
  . = ALIGN(4096);
  .text : AT(ADDR(.text) - KBASE) {
    *(.text .text.*)
  } :text

  /* Read-only data */
  . = ALIGN(4096);
  .rodata : AT(ADDR(.rodata) - KBASE) {
    *(.rodata .rodata.*)
  } :text

  /* Writable data */
  . = ALIGN(4096);
  .data : AT(ADDR(.data) - KBASE) {
    *(.data .data.*)
  } :data

  /* BSS: no file bytes, just virtual space */
  . = ALIGN(4096);
  __bss_start = .;
  .bss (NOLOAD) : {
    *(.bss .bss.* COMMON)
    *(.bss.boot)
  } :data
  __bss_end = .;

  /* Useful symbols */
  __kernel_base     = KBASE;
  __phys_load_base  = PLOAD;
  __virt_start      = ADDR(.text);
  __phys_start      = LOADADDR(.text);
  __virt_end        = .;
  __phys_end        = LOADADDR(.data) + SIZEOF(.data);

  /* Drop unwind info */
  /DISCARD/ : { *(.eh_frame) *(.eh_frame_hdr) }
}
