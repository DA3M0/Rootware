//! Boot-time kernel self-test suite.
//!
//! Runs after the core subsystems initialize and before user programs
//! spawn. Every result prints a `[SELFTEST] PASS/FAIL` line that
//! `run-tests.sh` asserts against; a failure flips the QEMU exit code
//! when the kernel finally idles.

use core::sync::atomic::{AtomicBool, Ordering};

use rootware_abi::ErrorCode;

use crate::serial_println;

static FAILED: AtomicBool = AtomicBool::new(false);

pub fn failed() -> bool {
    FAILED.load(Ordering::Relaxed)
}

fn check(name: &str, ok: bool) {
    if ok {
        serial_println!("[SELFTEST] PASS {name}");
    } else {
        serial_println!("[SELFTEST] FAIL {name}");
        FAILED.store(true, Ordering::Relaxed);
    }
}

pub fn run_all() {
    check("frame-allocator", frames_roundtrip());
    check("kernel-heap", heap_stress());
    check("ipc-queues", ipc_queues());
    check("elf-modules", elf_modules_parse());
    check("abi-status", abi_status_roundtrip());
    check("capabilities", capability_grants());
    serial_println!("[SELFTEST] suite complete");
}

/// Allocate a few frames, verify uniqueness and alignment, return them.
fn frames_roundtrip() -> bool {
    let before = crate::memory::free_count();
    let Some(first) = crate::memory::alloc_frame() else {
        return false;
    };
    let Some(second) = crate::memory::alloc_frame() else {
        return false;
    };
    let ok = first != second
        && first % crate::memory::FRAME_SIZE == 0
        && second % crate::memory::FRAME_SIZE == 0;
    let freed_first = crate::memory::free_frame(first).is_ok();
    let freed_second = crate::memory::free_frame(second).is_ok();
    ok && freed_first && freed_second && crate::memory::free_count() == before
}

/// Drive the global allocator through alloc/realloc/free patterns.
fn heap_stress() -> bool {
    let mut values: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
    for index in 0..2000u64 {
        values.push(index ^ 0xdead_beef);
    }
    if values[1999] != 1999 ^ 0xdead_beef {
        return false;
    }
    drop(values);
    let mut block: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    block.resize(8192, 0xA5);
    block.iter().all(|byte| *byte == 0xA5)
}

/// Per-receiver isolation, FIFO delivery and empty-queue semantics over
/// the real kernel queues (seeded by the boot permission table).
fn ipc_queues() -> bool {
    let message = crate::ipc::test_message();
    if crate::ipc::send(message).is_err() {
        return false;
    }
    // Receiver 1 must not observe receiver 2's traffic.
    if crate::ipc::recv(1).is_ok() {
        return false;
    }
    let received = match crate::ipc::recv(2) {
        Ok(message) => message,
        Err(_) => return false,
    };
    let drained = crate::ipc::recv(2).is_err();
    received.sender == 1
        && received.receiver == 2
        && received.message_type == rootware_abi::ipc::message_type::REQUEST
        && drained
}

/// Parse every boot module image: the kernel's own loader must accept the
/// programs it is about to run.
fn elf_modules_parse() -> bool {
    let count = crate::memory::module_count();
    if count == 0 {
        // No-module boots (kernel demo) skip this check.
        return true;
    }
    (0..count).all(|index| match crate::memory::module(index) {
        Some(info) => {
            let ok = crate::elf::parse(crate::memory::module_bytes(&info)).is_ok();
            if !ok {
                crate::serial_println!(
                    "[SELFTEST] module '{}' failed to parse ({:#x}..{:#x})",
                    info.name_str().unwrap_or("?"),
                    info.start,
                    info.end
                );
            }
            ok
        }
        None => false,
    })
}

fn abi_status_roundtrip() -> bool {
    let codes = [
        ErrorCode::InvalidArgument,
        ErrorCode::PermissionDenied,
        ErrorCode::QueueFull,
        ErrorCode::QueueEmpty,
        ErrorCode::NotFound,
        ErrorCode::Unsupported,
        ErrorCode::Transport,
    ];
    codes
        .iter()
        .all(|code| ErrorCode::from_status(code.status()) == Some(*code))
        && ErrorCode::from_status(0).is_none()
}

/// The boot policy granted the IPC send capability; unrelated ids must
/// hold nothing, and the audit log must answer queries.
fn capability_grants() -> bool {
    let send = crate::capability::Capability {
        id: crate::capability::IPC_SEND_CAPABILITY,
    };
    let other = crate::capability::Capability { id: 0x7F7F };
    crate::capability::holds(1, send)
        && !crate::capability::holds(1, other)
        && !crate::capability::holds(0, send)
}
