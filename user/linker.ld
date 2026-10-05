ENTRY(_start)

/* Rootware user programs are static ELF64 executables linked at the
 * fixed user base; the kernel maps each PT_LOAD segment into a fresh
 * per-process address space. */

SECTIONS {
    . = 0x40000000;

    .text : ALIGN(4K) {
        *(.text*)
    }

    .rodata : ALIGN(4K) {
        *(.rodata*)
    }

    .data : ALIGN(4K) {
        *(.data*)
        *(.got*)
        *(.data.rel.ro*)
        *(.sdata*)
    }

    .bss : ALIGN(4K) {
        *(.bss*)
        *(.sbss*)
        *(COMMON)
    }

    /DISCARD/ : {
        *(.eh_frame*)
        *(.comment)
        *(.note*)
        *(.interp*)
        *(.gnu*)
    }
}
