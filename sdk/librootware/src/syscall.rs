//! Rootware kernel syscall transport.
//!
//! Rootware uses the x86_64 `syscall` instruction with the syscall number in
//! `rax`. Pointer arguments refer to the fixed ABI structures in [`crate::ipc`].
//! The kernel returns zero on success and a negative [`ErrorCode`] value on
//! failure.

use rootware_abi::syscall::{SYS_IPC_RECEIVE, SYS_IPC_REPLY, SYS_IPC_SEND};

use crate::error::{Error, ErrorCode, Result};
use crate::ipc::{Message, Transport, PAYLOAD_SIZE};

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

/// Raw syscall: `Ok(status)` carries non-negative results (data such as
/// versions or pids), `Err` decodes negative statuses into [`ErrorCode`].
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub(crate) unsafe fn invoke_raw(number: u64, first: u64, second: u64) -> Result<i64> {
    let status: i64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inlateout("rax") number as i64 => status,
            in("rdi") first,
            in("rsi") second,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack)
        );
    }
    match ErrorCode::from_status(status) {
        None => Ok(status),
        Some(code) => Err(Error::new(code)),
    }
}

#[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
pub(crate) unsafe fn invoke_raw(_number: u64, _first: u64, _second: u64) -> Result<i64> {
    Err(Error::new(ErrorCode::Unsupported))
}

impl Transport for SyscallTransport {
    fn send(&mut self, message: Message) -> Result<()> {
        #[cfg(all(target_os = "none", target_arch = "x86_64"))]
        {
            // SAFETY: `message` remains live for the duration of the syscall.
            return unsafe {
                invoke_raw(SYS_IPC_SEND, (&message as *const Message) as u64, 0)
            }
            .map(|_| ());
        }
        #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
        {
            let _ = message;
            Err(Error::new(ErrorCode::Unsupported))
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
            // On Rootware this blocks the process until a message arrives.
            unsafe {
                invoke_raw(
                    SYS_IPC_RECEIVE,
                    receiver as u64,
                    (&mut message as *mut Message) as u64,
                )?;
            }
            Ok(message)
        }
        #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
        {
            let _ = receiver;
            Err(Error::new(ErrorCode::Unsupported))
        }
    }

    fn reply(&mut self, request: &Message, payload: [u8; PAYLOAD_SIZE]) -> Result<()> {
        if request.receiver == 0 || request.sender == 0 {
            return Err(Error::new(ErrorCode::InvalidArgument));
        }

        #[cfg(all(target_os = "none", target_arch = "x86_64"))]
        {
            // SAFETY: both values remain live for the duration of the syscall.
            return unsafe {
                invoke_raw(
                    SYS_IPC_REPLY,
                    (request as *const Message) as u64,
                    (&payload as *const [u8; PAYLOAD_SIZE]) as u64,
                )
            }
            .map(|_| ());
        }
        #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
        {
            let _ = payload;
            Err(Error::new(ErrorCode::Unsupported))
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
