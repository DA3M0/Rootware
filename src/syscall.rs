//! Kernel syscall dispatch (ABI v5).
//!
//! Runs on the current process's kernel stack inside its address space.
//! Every user pointer is validated against the caller's address space
//! before the kernel touches it.

use core::mem::size_of;

use rootware_abi::capability::capability_kind;
use rootware_abi::rkm::RkmModule;
use rootware_abi::syscall::{
    SYS_CAP_REQUEST, SYS_CONSOLE_READ, SYS_CONSOLE_WRITE, SYS_EXIT, SYS_IPC_RECEIVE, SYS_IPC_REPLY,
    SYS_IPC_SEND, SYS_MODULE_LIST, SYS_MODULE_REGISTER, SYS_SPAWN, SYS_VERSION,
};
use rootware_abi::{ErrorCode, Message};

use crate::ipc;
use crate::vmem;

/// Longest program name accepted by SYS_SPAWN (module names are capped at
/// 16 bytes including the NUL).
const MAX_NAME_LEN: usize = 16;

/// Read a `Copy` value out of caller memory after validating the range.
fn user_read<T: Copy>(ptr: u64) -> Result<T, ErrorCode> {
    let len = size_of::<T>() as u64;
    if ptr == 0 || !vmem::is_user_accessible(ptr, len) {
        return Err(ErrorCode::InvalidArgument);
    }
    Ok(unsafe { core::ptr::read_volatile(ptr as *const T) })
}

/// Write a `Copy` value into caller memory after validating the range.
fn user_write<T: Copy>(ptr: u64, value: T) -> Result<(), ErrorCode> {
    let len = size_of::<T>() as u64;
    if ptr == 0 || !vmem::is_user_accessible(ptr, len) {
        return Err(ErrorCode::InvalidArgument);
    }
    unsafe {
        core::ptr::write_volatile(ptr as *mut T, value);
    }
    Ok(())
}

/// Read a bounded byte buffer out of caller memory.
fn user_read_bytes(ptr: u64, len: u64, buffer: &mut [u8]) -> Result<usize, ErrorCode> {
    if ptr == 0 || len == 0 || len as usize > buffer.len() {
        return Err(ErrorCode::InvalidArgument);
    }
    if !vmem::is_user_accessible(ptr, len) {
        return Err(ErrorCode::InvalidArgument);
    }
    unsafe {
        core::ptr::copy_nonoverlapping(ptr as *const u8, buffer.as_mut_ptr(), len as usize);
    }
    Ok(len as usize)
}

/// Capability kinds a caller may acquire through SYS_CAP_REQUEST. The boot
/// policy trusts the first process (the client/init slot) with IPC send.
const CAP_WHITELIST: [(u16, u32); 1] = [(1, capability_kind::IPC_SEND)];

pub fn dispatch(number: u64, first: u64, second: u64) -> i64 {
    match number {
        SYS_IPC_SEND => match user_read::<Message>(first) {
            // The kernel, not the caller, owns the sender identity.
            Ok(mut message) => {
                message.sender = crate::process::current_id();
                match ipc::send(message) {
                    Ok(()) => 0,
                    Err(code) => code.status(),
                }
            }
            Err(code) => code.status(),
        },
        SYS_IPC_RECEIVE => match receive_pid(first) {
            Ok(message) => match user_write(second, message) {
                Ok(()) => 0,
                Err(code) => code.status(),
            },
            Err(code) => code.status(),
        },
        SYS_IPC_REPLY => match reply(first, second) {
            Ok(()) => 0,
            Err(code) => code.status(),
        },
        SYS_VERSION => rootware_abi::ABI_VERSION as i64,
        SYS_CONSOLE_WRITE => match console_write(first, second) {
            Ok(()) => 0,
            Err(code) => code.status(),
        },
        SYS_CONSOLE_READ => match console_read(first, second) {
            Ok(count) => count,
            Err(code) => code.status(),
        },
        SYS_EXIT => crate::process::exit_current(first as i64),
        SYS_SPAWN => match spawn(first, second) {
            Ok(pid) => pid as i64,
            Err(code) => code.status(),
        },
        SYS_CAP_REQUEST => match request_capability(first as u32) {
            Ok(()) => 0,
            Err(code) => code.status(),
        },
        SYS_MODULE_REGISTER => match register_module(first) {
            Ok(()) => 0,
            Err(code) => code.status(),
        },
        SYS_MODULE_LIST => match list_modules(first, second) {
            Ok(count) => count,
            Err(code) => code.status(),
        },
        _ => ErrorCode::Unsupported.status(),
    }
}

/// Blocking receive, bound to the caller's own queue. The requested
/// receiver id must match the caller's process id, so one process can
/// never read another process's messages.
fn receive_pid(requested: u64) -> Result<Message, ErrorCode> {
    let pid = crate::process::current_id();
    if requested != pid as u64 {
        return Err(ErrorCode::PermissionDenied);
    }
    let receiver = pid;
    loop {
        match ipc::recv(receiver) {
            Ok(message) => return Ok(message),
            Err(ErrorCode::QueueEmpty) => crate::process::block_current(receiver),
            Err(code) => return Err(code),
        }
    }
}

fn reply(request_ptr: u64, payload_ptr: u64) -> Result<(), ErrorCode> {
    let request: Message = user_read(request_ptr)?;
    if request.receiver == 0 || request.sender == 0 {
        return Err(ErrorCode::InvalidArgument);
    }
    let payload: [u8; ipc::PAYLOAD_SIZE] = user_read(payload_ptr)?;
    // Replies always originate from the calling process and carry the
    // RESPONSE type so the router sends them back to the requester.
    let response = Message {
        sender: crate::process::current_id(),
        receiver: request.sender,
        message_type: rootware_abi::ipc::message_type::RESPONSE,
        capability: request.capability,
        payload,
    };
    match ipc::send(response) {
        Ok(()) => Ok(()),
        Err(code) => Err(code),
    }
}

fn console_write(ptr: u64, len: u64) -> Result<(), ErrorCode> {
    if len > 4096 {
        crate::serial_println!("[DBG] console: len too big {}", len);
        return Err(ErrorCode::InvalidArgument);
    }
    let mut buffer = [0u8; 4096];
    match user_read_bytes(ptr, len, &mut buffer) {
        Ok(copied) => {
            for byte in &buffer[..copied] {
                crate::serial::write_byte(*byte);
            }
            Ok(())
        }
        Err(code) => {
            crate::serial_println!("[DBG] console: read failed ptr={:#x} len={} code={:?}", ptr, len, code);
            Err(code)
        }
    }
}

/// Blocking console read: waits for at least one byte in the kernel
/// input ring, then copies as many bytes as are available (up to `len`)
/// into caller memory. Every retry holds interrupts off while deciding
/// between "byte ready" and "block", and the process is marked Blocked
/// before IF is restored, so the timer poll can never slip between
/// "ring empty" and "parked" and lose its wake-up.
fn console_read(ptr: u64, len: u64) -> Result<i64, ErrorCode> {
    if len == 0 || len > 512 || !vmem::is_user_accessible(ptr, len) {
        return Err(ErrorCode::InvalidArgument);
    }
    let out = ptr as *mut u8;
    let count = loop {
        crate::console::disable_interrupts();
        match crate::console::pop() {
            Some(first) => {
                let mut count = 0usize;
                let mut byte = first;
                loop {
                    unsafe {
                        out.add(count).write_volatile(byte);
                    }
                    count += 1;
                    if count == len as usize {
                        break;
                    }
                    match crate::console::pop() {
                        Some(next) => byte = next,
                        None => break,
                    }
                }
                break count;
            }
            None => {
                crate::process::block_current(crate::console::CONSOLE_WAIT);
                // Resumed: schedule() re-enabled interrupts. Loop back
                // and re-check the ring under cli.
            }
        }
    };
    crate::console::enable_interrupts();
    Ok(count as i64)
}

fn spawn(name_ptr: u64, name_len: u64) -> Result<u16, ErrorCode> {
    let mut buffer = [0u8; MAX_NAME_LEN];
    let len = user_read_bytes(name_ptr, name_len, &mut buffer)?;
    let name = core::str::from_utf8(&buffer[..len]).map_err(|_| ErrorCode::InvalidArgument)?;
    crate::process::spawn_by_name(name)
}

fn request_capability(kind: u32) -> Result<(), ErrorCode> {
    if kind == 0 {
        return Err(ErrorCode::InvalidArgument);
    }
    let pid = crate::process::current_id();
    if CAP_WHITELIST.contains(&(pid, kind)) {
        crate::capability::grant(pid, crate::capability::Capability { id: kind })
            .map_err(|_| ErrorCode::QueueFull)
    } else {
        Err(ErrorCode::PermissionDenied)
    }
}

/// Registers the calling driver process as an RKM module. The caller
/// fills name/version/kind in the descriptor; the kernel fills pid and
/// state and writes the stored descriptor back to the same memory, so
/// the driver learns its own process id.
fn register_module(descriptor_ptr: u64) -> Result<(), ErrorCode> {
    let descriptor: RkmModule = user_read(descriptor_ptr)?;
    let pid = crate::process::current_id();
    let stored = crate::rkm::register(pid, descriptor)?;
    // Registration IS the driver privilege: a registered module may
    // hold IPC_SEND (to answer requests) and gets its client lanes
    // opened in the permission table.
    let _ = crate::capability::grant(
        pid,
        crate::capability::Capability {
            id: crate::capability::IPC_SEND_CAPABILITY,
        },
    );
    crate::ipc::allow_driver(pid);
    crate::serial_println!(
        "[RKM] module '{}' v{} ({}) registered as pid {}",
        stored.name_str().unwrap_or("?"),
        stored.version_str().unwrap_or("?"),
        kind_name(stored.kind),
        pid
    );
    user_write(descriptor_ptr, stored)
}

fn kind_name(kind: u8) -> &'static str {
    match kind {
        rootware_abi::rkm::module_kind::NATIVE => "native",
        rootware_abi::rkm::module_kind::LINUX => "linux",
        _ => "unknown",
    }
}

/// Copies up to `capacity` registry entries into caller memory and
/// returns the total number of stored entries, so a caller with a small
/// buffer can retry with a bigger one.
fn list_modules(buffer_ptr: u64, capacity: u64) -> Result<i64, ErrorCode> {
    if buffer_ptr == 0 || capacity == 0 {
        return Err(ErrorCode::InvalidArgument);
    }
    let total = crate::rkm::count();
    let written = (capacity as usize).min(total);
    if written > 0 {
        let bytes = (written * size_of::<RkmModule>()) as u64;
        if !vmem::is_user_accessible(buffer_ptr, bytes) {
            return Err(ErrorCode::InvalidArgument);
        }
    }
    for index in 0..written {
        let Some(entry) = crate::rkm::entry(index) else {
            break;
        };
        let slot = buffer_ptr + (index * size_of::<RkmModule>()) as u64;
        user_write(slot, entry)?;
    }
    Ok(total as i64)
}
