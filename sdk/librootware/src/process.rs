//! Process lifecycle syscalls.

use crate::error::{Error, ErrorCode, Result};
use rootware_abi::syscall::{SYS_EXIT, SYS_SPAWN};

/// Terminates the calling process with `code`. Never returns on Rootware;
/// on other targets this panics, mirroring a `std` process without an OS.
pub fn exit(code: i32) -> ! {
    #[cfg(all(target_os = "none", target_arch = "x86_64"))]
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") SYS_EXIT,
            in("rdi") code as u64,
            options(noreturn, nostack)
        );
    }
    #[cfg(not(all(target_os = "none", target_arch = "x86_64")))]
    {
        let _ = code;
        panic!("process::exit has no kernel to return to on this target");
    }
}

/// Spawns the boot module registered under `name` (up to 15 bytes) as a
/// new process and returns its process id.
pub fn spawn(name: &str) -> Result<u16> {
    if name.is_empty() || name.len() >= 16 {
        return Err(Error::new(ErrorCode::InvalidArgument));
    }
    let pid = unsafe { crate::syscall::invoke_raw(SYS_SPAWN, name.as_ptr() as u64, name.len() as u64)? };
    Ok(pid as u16)
}
