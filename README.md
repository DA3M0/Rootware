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

## Status

Beta — functional kernel. Virtual memory, scheduler, IPC, and first userspace service are working.

## Architecture

Rootware has three layers:

### Kernel (Apache 2.0)

- Scheduler (round-robin, real context switching)
- Virtual memory (4-level page tables, CR3 switching)
- Physical memory management (bitmap allocator)
- IPC + security layer (message passing, configurable permission checks, audit log)
- Interrupt/exception handling (GDT, IDT, APIC timer)
- Architecture abstraction (x86_64)

Around 15,000–20,000 lines, zero third-party dependencies.

### Userspace (Mixed Licenses)

Built by the community, largely reusing existing components:

- Filesystem: ext4
- Drivers: Linux drivers (via rkm)
- Shell: Rash / Brush
- Coreutils: uutils
- Python: RustPython
- Editor: Helix

## Build & Run

### Dependencies

- Rust stable
- QEMU
- GRUB2 tools (`grub2-mkrescue`)
- NASM

## Beta roadmap

- [x] Beta 2: configurable IPC permission rules
- [ ] Beta 3: capability model
- [ ] Beta 4: message types and routing
- [ ] Beta 5: persistent audit log and queries
- [ ] Beta 6: IPC performance and stability
- [ ] Beta 7: frozen ABI and end-to-end validation
