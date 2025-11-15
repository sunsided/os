OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

PHDRS {
  text PT_LOAD FLAGS(5);   /* R X */
  data PT_LOAD FLAGS(6);   /* R W */
}

SECTIONS {
  . = SEGMENT_START("text-segment", 0x400000);

  .text : ALIGN(0x1000) {
    *(.text .text.*)
  } :text

  .rodata : ALIGN(0x1000) {
    *(.rodata .rodata.*)
  } :text

  .data : ALIGN(0x1000) {
    *(.data .data.*)
  } :data

  .bss (NOLOAD) : ALIGN(0x1000) {
    *(.bss .bss.* COMMON)
  } :data

  /DISCARD/ : { *(.eh_frame .eh_frame_hdr) }
}
