/* kernel.ld — fixed EXEC layout */
OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start_kernel)

PHDRS {
  text PT_LOAD FLAGS(5);   /* R+X */
  data PT_LOAD FLAGS(6);   /* R+W */
}

SECTIONS
{
  . = 1M;  /* chosen VMA; your loader must map here */

  /* Text */
  . = ALIGN(4096);
  .text : {
    *(.text .text.*)
  } :text

  /* Read-only data */
  . = ALIGN(4096);
  .rodata : {
    *(.rodata .rodata.*)
  } :text

  /* Writable data */
  . = ALIGN(4096);
  .data : {
    *(.data .data.*)
  } :data

  /* BSS (zero-initialized) */
  . = ALIGN(4096);
  .bss (NOLOAD) : {
    __bss_start = .;
    *(.bss .bss.* COMMON)
    /* If you use a boot stack section: *(.bss.boot) */
    __bss_end = .;
  } :data

  /* Drop unwind info */
  /DISCARD/ : { *(.eh_frame) *(.eh_frame_hdr) }
}
