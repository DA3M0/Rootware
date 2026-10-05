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
| Target | x86_64-unknown-none (kernel + user programs) |
| Kernel License | Apache 2.0 |

## Status

**1.0 — first stable release.** ABI v4 frozen; full process model with per-process address spaces; user programs loaded from boot modules and talking IPC end to end; stability gated by host unit tests plus an automated QEMU boot selftest (`run-tests.sh`).

## Architecture

Rootware has three layers:

### Kernel (Apache 2.0)

- Process model: 8-slot process table, spawn/exit, cooperative round-robin scheduling with real context switches and blocking IPC
- Virtual memory: 4-level paging, one address space per process (user region at 0x40000000), CR3 switching, full-RAM identity map for the kernel
- Physical memory: bitmap frame allocator over the Multiboot2 memory map, plus a kernel heap (`alloc` collections)
- IPC: per-receiver bounded queues, typed routing, capability checks, audit log; sender identity is kernel-owned
- Syscalls (ABI v4, frozen): IPC send/receive/reply, version handshake, console write, exit, spawn, capability request — entered through `syscall`/`sysret` with per-process kernel stacks and validated user pointers
- Interrupts: full exception frames, kernel GDT + TSS (RSP0, IST), Ring 3 faults kill the offending process instead of halting the machine

### SDK (`librootware`, Apache 2.0)

- `Transport` abstraction with a syscall-backed implementation and an in-memory test double
- Console output, process spawn/exit, capability requests, runtime ABI handshake
- `Service` lifecycle framework; no_std mode ships crt0 and a panic handler

### Userspace (boot modules)

`user/` ships three example programs built as static ELF64 binaries loaded by the kernel's ELF loader: `hello` (console), `echo-service` (IPC service, pid 2), `ipc-client` (handshake + capability + echo round trip, pid 1). Around 4,600 lines of Rust in total, zero third-party dependencies.

The larger userspace components (filesystem, drivers, shell, coreutils) remain community-reuse plans for after 1.0.

## Build & Run

### Dependencies

- Rust stable (rustup toolchain; the bare-metal target: `rustup target add x86_64-unknown-none`)
- QEMU
- GRUB2 tools (`grub2-mkrescue`)
- NASM

### Commands

```sh
cargo test --workspace   # host unit tests (kernel logic + SDK + simulator)
./build.sh               # build kernel + user programs, create rootware.iso, boot QEMU
./run-tests.sh           # full stability gate: host tests + QEMU selftest + end-to-end IPC
```

A successful gate boots the kernel, prints `[SELFTEST] PASS` for every kernel selftest, runs the ABI handshake and echo round trip in Ring 3, and exits QEMU cleanly.

## Roadmap

Beta milestones, all shipped:

- [x] Beta 2: configurable IPC permission rules
- [x] Beta 3: capability model
- [x] Beta 4: message types and routing
- [x] Beta 5: persistent audit log and queries
- [x] Beta 6: IPC performance and stability
- [x] Beta 7: frozen ABI and end-to-end validation

1.0:

- [x] ABI v4 freeze (shared `rootware-abi` crate, runtime version handshake)
- [x] Physical frame allocator, kernel heap, per-process address spaces
- [x] Process table with spawn/exit and blocking IPC
- [x] Kernel GDT/TSS, hardened syscall entry, faulting processes isolated
- [x] ELF loader + Multiboot2 boot modules, user build pipeline (crt0, linker script)
- [x] SDK: console, process, capability, version handshake
- [x] Stability gate: host tests + QEMU selftest suite + user-space round trip
