//! Rootware kernel syscall transport.
//!
//! Rootware uses the x86_64 `syscall` instruction with the syscall number in
//! `rax`. Pointer arguments refer to the fixed ABI structures in [`crate::ipc`].
//! The kernel returns zero on success and a negative [`ErrorCode`] value on
//! failure.

use crate::error::{Error, ErrorCode, Result};
use crate::ipc::{Message, Transport, PAYLOAD_SIZE};

/// Send an IPC message.
pub const SYS_IPC_SEND: u64 = 1;
/// Receive an IPC message for a service.
pub const SYS_IPC_RECEIVE: u64 = 2;
/// Reply to an IPC request.
pub const SYS_IPC_REPLY: u64 = 3;

/// Transport that delegates IPC operations to the Rootware kernel.
///
/// On non-Rootware targets this type is still available so application code
/// can compile unchanged, but every operation returns [`ErrorCode::Unsupported`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SyscallTransport;

impl SyscallTransport {
    /// Creates a kernel-backed transport.
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn status_result(status: i64) -> Result<()> {
    if status == 0 {
        return Ok(());
    }

    let code = if status < 0 { -status } else { status };
    let error = match code as u32 {
        1 => ErrorCode::InvalidArgument,
        2 => ErrorCode::PermissionDenied,
        3 => ErrorCode::QueueFull,
        4 => ErrorCode::QueueEmpty,
        5 => ErrorCode::NotFound,
        6 => ErrorCode::Unsupported,
        _ => ErrorCode::Transport,
    };
    Err(Error::new(error))
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn invoke(number: u64, first: u64, second: u64) -> i64 {
    let result: i64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") number as i64 => result,
            in("rdi") first,
            in("rsi") second,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }
    result
}

#[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
fn unsupported<T>() -> Result<T> {
    Err(Error::new(ErrorCode::Unsupported))
}

impl Transport for SyscallTransport {
    fn send(&mut self, message: Message) -> Result<()> {
        #[cfg(all(target_os = "none", target_arch = "x86_64"))]
        {
            // SAFETY: `message` remains live for the duration of the syscall.
            return status_result(unsafe {
                invoke(SYS_IPC_SEND, (&message as *const Message) as u64, 0)
            });
        }
        #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
        {
            let _ = message;
            unsupported()
        }
    }

    fn receive(&mut self, receiver: u16) -> Result<Message> {
        #[cfg(all(target_os = "none", target_arch = "x86_64"))]
        {
            let mut message = Message::new(
                0,
                receiver,
                0,
                crate::capability::Capability { id: 0 },
                [0; PAYLOAD_SIZE],
            );
            // SAFETY: `message` is writable and remains live for the syscall.
            status_result(unsafe {
                invoke(
                    SYS_IPC_RECEIVE,
                    receiver as u64,
                    (&mut message as *mut Message) as u64,
                )
            })?;
            Ok(message)
        }
        #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
        {
            let _ = receiver;
            unsupported()
        }
    }

    fn reply(&mut self, request: &Message, payload: [u8; PAYLOAD_SIZE]) -> Result<()> {
        if request.receiver == 0 || request.sender == 0 {
            return Err(Error::new(ErrorCode::InvalidArgument));
        }

        #[cfg(all(target_os = "none", target_arch = "x86_64"))]
        {
            // SAFETY: both values remain live for the duration of the syscall.
            return status_result(unsafe {
                invoke(
                    SYS_IPC_REPLY,
                    (request as *const Message) as u64,
                    (&payload as *const [u8; PAYLOAD_SIZE]) as u64,
                )
            });
        }
        #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
        {
            let _ = payload;
            unsupported()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability;

    #[test]
    fn unsupported_on_host() {
        let mut transport = SyscallTransport::new();
        let message = Message::new(1, 2, 1, Capability { id: 1 }, [0; PAYLOAD_SIZE]);
        assert_eq!(
            transport.send(message),
            Err(Error::new(ErrorCode::Unsupported))
        );
        assert_eq!(
            transport.receive(2),
            Err(Error::new(ErrorCode::Unsupported))
        );
    }

    #[test]
    fn reply_rejects_invalid_routes_before_syscall() {
        let mut transport = SyscallTransport::new();
        let request = Message::new(0, 2, 1, Capability { id: 1 }, [0; PAYLOAD_SIZE]);
        assert_eq!(
            transport.reply(&request, [0; PAYLOAD_SIZE]),
            Err(Error::new(ErrorCode::InvalidArgument))
        );
    }
}
