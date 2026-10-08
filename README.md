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

**2.0 — interactive era begins.** ABI v5 (v4 deprecated, compatibility window kept for pre-2.0 binaries); console input reaches the kernel and the first interactive shell ships; full process model with per-process address spaces; user programs loaded from boot modules and talking IPC end to end; stability gated by host unit tests plus an automated QEMU boot that drives the shell (`run-tests.sh`). The 2.1/2.2 series plans — process manager, kernel architecture work, interactive-system rollout, filesystem in the last 2.2 minor — live in [docs/路线图.md](docs/路线图.md).

## Architecture

Rootware has three layers:

### Kernel (Apache 2.0)

- Process model: 8-slot process table, spawn/exit, cooperative round-robin scheduling with real context switches and blocking IPC
- Virtual memory: 4-level paging, one address space per process (user region at 0x40000000), CR3 switching, full-RAM identity map for the kernel
- Physical memory: bitmap frame allocator over the Multiboot2 memory map, plus a kernel heap (`alloc` collections)
- IPC: per-receiver bounded queues, typed routing, capability checks, audit log; sender identity is kernel-owned
- Syscalls (ABI v5, append-only since v4): IPC send/receive/reply, version handshake, console write/read, exit, spawn, capability request, RKM module register/list — entered through `syscall`/`sysret` with per-process kernel stacks and validated user pointers. Console input is a timer-polled UART ring buffer; the scheduler hlt-idles instead of powering off while a shell waits
- RKM driver framework: user-space drivers register into a kernel registry (native Rust via the SDK, or Linux-style C drivers via the compat shim); registration grants the IPC send capability and opens the driver's client lanes
- Interrupts: full exception frames, kernel GDT + TSS (RSP0, IST), Ring 3 faults kill the offending process instead of halting the machine

### SDK (`librootware`, Apache 2.0)

- `Transport` abstraction with a syscall-backed implementation and an in-memory test double
- Console output and line-edited input (`read_line` with echo, backspace, escape-sequence swallowing), process spawn/exit, capability requests, runtime ABI handshake over the v4–v5 compatibility window
- RKM driver support: `rkm::register`/`rkm::list`, a `Driver` trait mirroring `Service`
- `Service` lifecycle framework; no_std mode ships crt0 and a panic handler

### Userspace (boot modules)

`user/` ships example components built as static ELF64 binaries loaded by the kernel's ELF loader: `hello` (console), `echo-service` (IPC service, pid 2), `ipc-client` (handshake + capability + echo round trip + RKM driver read, pid 1), `rkm-driver` (native RKM counter device), `zero-driver` (a Linux-compatible C driver ported from `/dev/zero`), `mixed-demo` (one program statically linking Rust + C + C++ + Zig via the C ABI), and `libcrc` (a `lib`-kind component consumed through `[link] libs`). `shell` is the interactive serial console (`rootware> ` prompt, line editing, `help`/`echo`/`clear`/`abi`/`modules`/`run`/`exit`) reading through the kernel's console-input syscall.

The build system discovers every `user/*` component automatically (manifest `program.toml` optional for pure-Rust crates), supports the kinds `program` / `service` / `driver` / `lib`, and the kernel auto-spawns all boot modules (pinned: client pid 1, echo pid 2) — adding a component is one `./build.sh new <kind> <name>` away.

Console input is provided by `SYS_CONSOLE_READ` (ABI v5): the APIC timer polls the UART at 100 Hz into a kernel ring buffer and wakes the process blocked on the read. ABI v4 is deprecated since 2.0 — the handshake window `[ABI_COMPAT_MIN, ABI_VERSION]` keeps pre-2.0 binaries running, while all new code targets v5. The larger userspace components (filesystem, coreutils) are scheduled for the 2.2 series (see docs/路线图.md); drivers have a real framework (RKM) to land in.

## Build & Run

### Dependencies

- Rust stable (rustup toolchain; the bare-metal target: `rustup target add x86_64-unknown-none`)
- `cc` (gcc), `g++` and `zig` for the multi-language components
- QEMU
- GRUB2 tools (`grub2-mkrescue`)
- NASM

### Commands

```sh
./build.sh               # discover user/ components, build kernel + programs (Rust/C/C++/Zig), create rootware.iso, boot QEMU in a window (interactive console: View -> serial0 / Ctrl-Alt-3)
./build.sh list          # show discovered components (kind / entry / languages)
./build.sh new <kind> <name>   # instantiate a new program|service|driver|lib from templates
./build.sh build|iso|run|test|clean
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
