ENTRY(_start)

SECTIONS {
    . = 0x100000;

    .multiboot_header : {
        KEEP(*(.multiboot_header))
    }

    .text : ALIGN(4K) {
        *(.text*)
    }

    .rodata : ALIGN(4K) {
        *(.rodata*)
    }

    .data : ALIGN(4K) {
        *(.data*)
    }

    .bss : ALIGN(4K) {
        *(.bss*)
    }

    . = ALIGN(4K);
    . += 128K;
    . = ALIGN(4K);

    .requests : ALIGN(4K) {
        KEEP(*(.requests))
    }
}
