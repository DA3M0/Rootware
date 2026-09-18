section .multiboot_header
align 8
multiboot_header_start:
    dd 0xe85250d6                ; Multiboot2 magic
    dd 0                         ; architecture 0 (i386 protected mode)
    dd multiboot_header_end - multiboot_header_start
    dd 0x100000000 - (0xe85250d6 + (multiboot_header_end - multiboot_header_start))

    ; Information request: memory map (type 6).
    dw 1                         ; information request tag
    dw 0                         ; flags
    dd 16                        ; tag size, including padding
    dd 6                         ; requested information type
    dd 0                         ; padding

    ; End tag.
    dw 0
    dw 0
    dd 8
multiboot_header_end:

section .text
global _start
extern rust_start
_start:
    ; GRUB supplies the Multiboot2 information address in EBX.
    mov [mb_info_save], rbx

    ; Zero-extend the 32-bit physical address before calling Rust.
    mov edi, ebx
    mov rsp, stack_top
    call rust_start
    hlt

section .bss
align 8
global mb_info_save
mb_info_save: resq 1

align 16
stack_bottom:
    resb 64 * 1024
stack_top:
