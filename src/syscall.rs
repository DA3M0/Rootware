//! Kernel syscall dispatch (ABI v4).
//!
//! Runs on the current process's kernel stack inside its address space.
//! Every user pointer is validated against the caller's address space
//! before the kernel touches it.

use core::mem::size_of;

use rootware_abi::capability::capability_kind;
use rootware_abi::syscall::{
    SYS_CAP_REQUEST, SYS_CONSOLE_WRITE, SYS_EXIT, SYS_IPC_RECEIVE, SYS_IPC_REPLY, SYS_IPC_SEND,
    SYS_SPAWN, SYS_VERSION,
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
        SYS_EXIT => crate::process::exit_current(first as i64),
        SYS_SPAWN => match spawn(first, second) {
            Ok(pid) => pid as i64,
            Err(code) => code.status(),
        },
        SYS_CAP_REQUEST => match request_capability(first as u32) {
            Ok(()) => 0,
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
        return Err(ErrorCode::InvalidArgument);
    }
    let mut buffer = [0u8; 4096];
    let copied = user_read_bytes(ptr, len, &mut buffer)?;
    for byte in &buffer[..copied] {
        crate::serial::write_byte(*byte);
    }
    Ok(())
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
