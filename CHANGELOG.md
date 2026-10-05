# Changelog

## 1.0.0 — first stable release (2026-10-06)

The first stable Rootware release: a frozen ABI, a full process model, a
user-space build pipeline and an automated stability gate. Everything below
is covered by `run-tests.sh` (host unit tests + QEMU boot selftests + an
end-to-end IPC round trip in Ring 3).

### ABI v4 (frozen)

- New shared `rootware-abi` crate is the single source of truth for the
  kernel/SDK interface: the 44-byte `#[repr(C)] Message`, capability kinds,
  error codes and the syscall number table. The kernel and SDK no longer
  maintain separate copies.
- Frozen syscalls: IPC_SEND/RECEIVE/REPLY (1–3, unchanged) plus VERSION (4),
  CONSOLE_WRITE (5), EXIT (6), SPAWN (7), CAP_REQUEST (8).
- Precise error semantics: the kernel reports PermissionDenied /
  QueueFull / NotFound / QueueEmpty / InvalidArgument / Unsupported /
  Transport instead of collapsing every send failure into one code.
- Runtime version handshake (`SYS_VERSION`); 1.0.x releases only extend the
  ABI additively.

### Kernel

- Bitmap physical frame allocator over the Multiboot2 memory map and a
  kernel heap enabling `alloc` collections.
- Per-process address spaces (user region 0x40000000) with CR3 switching and
  frame accounting; IPC queues are now per-receiver (no head-of-line
  blocking) with wake-on-send.
- Process table (8 slots) with spawn/exit replaces the hardcoded demo tasks;
  blocking IPC parks processes and wakes them on delivery.
- Kernel-owned GDT + TSS (RSP0, IST for double fault); full exception frames
  report faults and kill offending user processes instead of hanging the
  machine; legacy PIC masked; SSE enabled.
- Hardened syscall entry: correct sysret descriptors, FMASK, per-process
  kernel stacks, full user register preservation, and user-pointer
  validation against the caller's address space. Sender identity is
  kernel-owned; a process can only receive from its own queue.
- Minimal ELF64 loader; Multiboot2 boot modules are registered by name and
  relocated at boot before they can be clobbered.

### User-space

- `librootware` SDK 1.0: `Transport` + `SyscallTransport`, console output,
  `process::{exit, spawn}`, `capability::request_capability`, runtime ABI
  handshake; no_std mode ships crt0 (`rootware_main` convention) and a
  panic handler.
- User program pipeline: static ELF64 executables at 0x40000000 via a shared
  linker script, built outside the kernel workspace, installed into the ISO
  as GRUB modules.
- Three example programs run by the kernel at boot: `hello`, `echo-service`
  (pid 2), `ipc-client` (pid 1) — the client performs the handshake,
  requests its capability, and completes an echo round trip.

### Stability

- `run-tests.sh`: the release gate — workspace unit tests, QEMU boot with
  `[SELFTEST] PASS` for frame allocator / heap / IPC queues / ELF parsing /
  ABI status / capabilities, plus the user-space round trip and clean exit
  codes via `isa-debug-exit`.
- Kernel, SDK and user programs build warning-free on all targets.

### Breaking changes from Beta 7

- ABI version moved 3 → 4 (error semantics changed; syscalls 4–8 added).
- `Message::reply`-style responses now use the `RESPONSE` type so the router
  sends them back to the requester.
- The `scheduler`/`service` kernel demo modules were replaced by the process
  model; user programs come from boot modules.
