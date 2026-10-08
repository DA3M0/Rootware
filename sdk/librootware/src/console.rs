//! Console I/O through the kernel's serial console.
//!
//! Output copies the buffer into the kernel via `SYS_CONSOLE_WRITE`;
//! formatting is done locally through [`core::fmt::Write`] and never
//! allocates. Input blocks in `SYS_CONSOLE_READ` until the kernel input
//! ring has data.

use crate::error::{Error, ErrorCode, Result};
use rootware_abi::syscall::{SYS_CONSOLE_READ, SYS_CONSOLE_WRITE};

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

/// Reads up to `buffer.len()` bytes from the console, blocking until at
/// least one byte is available. Returns how many bytes were read.
pub fn read_bytes(buffer: &mut [u8]) -> Result<usize> {
    if buffer.is_empty() {
        return Ok(0);
    }
    unsafe {
        crate::syscall::invoke_raw(
            SYS_CONSOLE_READ,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
        )
    }
    .map(|count| count as usize)
}

/// Reads one byte, blocking until input is available.
pub fn read_byte() -> Result<u8> {
    let mut byte = [0u8; 1];
    read_bytes(&mut byte)?;
    Ok(byte[0])
}

/// Reads a line into `buffer` with interactive echo and editing:
/// printable characters append and echo, backspace erases, Enter (CR or
/// LF) ends the line, and ESC-prefixed terminal sequences (arrow keys)
/// are swallowed. Returns the line length without any terminator; a
/// full buffer ends the line early. The buffer is not NUL-terminated —
/// rely on the returned length.
pub fn read_line(buffer: &mut [u8]) -> Result<usize> {
    let mut len = 0usize;
    loop {
        let byte = read_byte()?;
        match byte {
            b'\r' | b'\n' => {
                let _ = write_str("\r\n");
                return Ok(len);
            }
            0x08 | 0x7F => {
                if len > 0 {
                    len -= 1;
                    let _ = write_str("\x08 \x08");
                }
            }
            0x1B => {
                // CSI sequence (arrow keys etc.): ESC, '[', parameter
                // bytes, then one final byte 0x40–0x7E that ends the
                // sequence. Swallow it whole — the introducer '[' and
                // the final byte included — and keep later input (such
                // as the Enter after an arrow key) intact.
                let next = read_byte()?;
                if next == b'[' {
                    while !(0x40..=0x7E).contains(&read_byte()?) {}
                }
            }
            0x20..=0x7E => {
                if len == buffer.len() {
                    let _ = write_str("\r\n");
                    return Ok(len);
                }
                buffer[len] = byte;
                len += 1;
                let _ = write_bytes(&[byte]);
            }
            _ => {}
        }
    }
}
