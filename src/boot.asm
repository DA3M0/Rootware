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
bits 32
_start:
    ; GRUB supplies the Multiboot2 information address in EBX.
    mov [mb_info_save], ebx

    ; Build a temporary identity map (0..4 MiB) and enter long mode.
    mov eax, bootstrap_pdp
    or eax, 0x3
    mov edi, bootstrap_pml4
    mov [edi], eax
    mov eax, bootstrap_pd
    or eax, 0x3
    mov edi, bootstrap_pdp
    mov [edi], eax
    mov eax, bootstrap_pd
    or eax, 0x3
    mov edi, bootstrap_pd
    mov [edi], eax
    mov dword [edi + 0], 0x00000083
    mov dword [edi + 8], 0x00200083

    ; Before the far jump, PAE interprets CR3 as a four-entry PDPT.
    mov eax, bootstrap_pdp
    mov cr3, eax
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax
    lgdt [gdt64.pointer]
    jmp 0x08:long_mode_start

bits 64
long_mode_start:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov rsp, stack_top
    sub rsp, 8
    mov edi, dword [mb_info_save]
    call rust_start
    hlt

section .bss
align 4096
bootstrap_pml4: resq 512
align 4096
bootstrap_pdp: resq 512
align 4096
bootstrap_pd: resq 512

align 8
global mb_info_save
mb_info_save: resq 1

align 16
stack_bottom:
    resb 64 * 1024
stack_top:

section .rodata
align 8
gdt64:
    dq 0
    dq 0x00af9a000000ffff
    dq 0x00af92000000ffff
.pointer:
    dw .pointer - gdt64 - 1
    dq gdt64
