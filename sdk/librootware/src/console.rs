//! Console output through the kernel's serial console.
//!
//! `SYS_CONSOLE_WRITE` copies the buffer into the kernel; formatting is
//! done locally through [`core::fmt::Write`] and never allocates.

use crate::error::{Error, ErrorCode, Result};
use rootware_abi::syscall::SYS_CONSOLE_WRITE;

/// Writes raw bytes to the console.
pub fn write_bytes(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    unsafe {
        crate::syscall::invoke_raw(SYS_CONSOLE_WRITE, bytes.as_ptr() as u64, bytes.len() as u64)
    }
    .map(|_| ())
}

/// Writes a string to the console.
pub fn write_str(text: &str) -> Result<()> {
    write_bytes(text.as_bytes())
}

struct Console;

impl core::fmt::Write for Console {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        write_str(text).map_err(|_| core::fmt::Error)
    }
}

/// Writes formatting arguments without allocating.
pub fn write_args(args: core::fmt::Arguments<'_>) -> Result<()> {
    use core::fmt::Write;
    let mut console = Console;
    console
        .write_fmt(args)
        .map_err(|_| Error::new(ErrorCode::Transport))
}

/// Prints a line to the console without allocating.
pub fn print_line(args: core::fmt::Arguments<'_>) {
    let _ = write_args(args);
    let _ = write_str("\n");
}
