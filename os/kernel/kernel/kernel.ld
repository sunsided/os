/* kernel.ld — higher-half kernel, loaded at 1 MiB */
OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start_kernel)

/* Tunables */
KERNEL_BASE = 0xffffffff80000000;   /* where the kernel executes (VMA) */
PHYS_LOAD   = 0x00100000;           /* where the image is loaded (LMA), e.g. 1 MiB */

PHDRS {
  text PT_LOAD FLAGS(5);   /* R+X */
  data PT_LOAD FLAGS(6);   /* R+W */
}

SECTIONS
{
  /* --- All section VMAs live in higher half --- */
  . = KERNEL_BASE + PHYS_LOAD;

  /* Text (VMA = KERNEL_BASE+..., LMA = PHYS_LOAD+offset) */
  . = ALIGN(4096);
  .text : AT(ADDR(.text) - KERNEL_BASE) {
    *(.text .text.*)
  } :text

  /* Read-only data */
  . = ALIGN(4096);
  .rodata : AT(ADDR(.rodata) - KERNEL_BASE) {
    *(.rodata .rodata.*)
  } :text

  /* Writable data */
  . = ALIGN(4096);
  .data : AT(ADDR(.data) - KERNEL_BASE) {
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
  __kernel_base     = KERNEL_BASE;
  __phys_load_base  = PHYS_LOAD;
  __virt_start      = ADDR(.text);
  __phys_start      = LOADADDR(.text);
  __virt_end        = .;
  __phys_end        = LOADADDR(.data) + SIZEOF(.data);

  /* Drop unwind info */
  /DISCARD/ : { *(.eh_frame) *(.eh_frame_hdr) }
}
