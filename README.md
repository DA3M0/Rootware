cat > README.md << 'EOF'
# Rootware

Rootware is a microkernel operating system written from scratch in Rust.

## Why

I'm 15, and I've been learning Rust for about a week. I wanted to learn by building a real operating system. Most tutorials either assume prior knowledge, use nightly, or rely on third-party libraries. So I decided to start from zero and write one myself.

## Tech Stack

| Item | Choice |
| :--- | :--- |
| Language | Rust (stable, 2024 Edition) |
| Dependencies | Zero third-party libraries |
| Bootloader | GRUB2 + Multiboot2 |
| Target | x86_64-unknown-none |
| Kernel License | Apache 2.0 |

## Architecture

Rootware has three layers:

### Kernel (Apache 2.0)

- Scheduler
- Memory management
- IPC + security layer
- Interrupt/exception handling
- Architecture abstraction (x86_64)

Around 15,000–20,000 lines, zero third-party dependencies. The security layer lives inside IPC — every message passes a permission check.

### Userspace (Mixed Licenses)

Largely reuses existing components:

- Filesystem: ext4
- Drivers: Linux drivers (via rkm)
- Shell: Rash / Brush
- Coreutils: uutils
- Python: RustPython
- Editor: Helix

Only the framework and system manager are custom, around 3,000–5,000 lines.

## Build & Run

### Dependencies

- Rust stable
- QEMU
- GRUB2 tools (`grub2-mkrescue`)
- NASM

## Roadmap

- [x] Alpha 5: physical memory management
- [x] Alpha 6: four-level virtual memory mapping and translation
- [x] Alpha 7: cooperative round-robin scheduler
- [x] Alpha 8: IPC, permissions, and audit logging
- [x] Beta: kernel module integration and first service handoff

The kernel ABI is represented by `#[repr(C)]` structures in the memory,
scheduler, and IPC modules. The current scheduler and IPC implementations
are fixed-size and allocation-free while the first userspace service ABI is
stabilized.
