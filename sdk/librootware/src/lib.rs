//! Safe, testable wrappers around the Rootware userspace ABI.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

pub mod capability;
pub mod console;
pub mod error;
pub mod ipc;
pub mod log;
pub mod process;
pub mod service;
pub mod syscall;
pub mod sys;

pub use error::{Error, Result};
pub use ipc::{Message, Transport};
pub use rootware_abi::{ABI_VERSION, ErrorCode};
pub use syscall::SyscallTransport;

/// Runtime ABI handshake target: programs linked against this SDK expect
/// a kernel implementing [`ABI_VERSION`].
pub use sys::{check_abi_version, kernel_abi_version};

// Program entry shim: the kernel loads static ELF64 executables and
// enters them at `_start` with the user stack pointer set. The shim
// aligns the stack, calls the program's `rootware_main`, and exits with
// its return code when it ever returns.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
core::arch::global_asm! {
    ".section .text._start",
    ".global _start",
    "_start:",
    "xor ebp, ebp",
    "and rsp, -16",
    "call rootware_main",
    "mov rdi, rax",
    "mov eax, {sys_exit}",
    "xor rsi, rsi",
    "syscall",
    "1: jmp 1b",
    sys_exit = const rootware_abi::syscall::SYS_EXIT,
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe extern "C" {
    fn rootware_main() -> i32;
}

/// Panic policy for user programs: report and exit. The kernel terminates
/// the process; the system keeps running.
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[panic_handler]
fn user_panic(info: &core::panic::PanicInfo<'_>) -> ! {
    use core::fmt::Write as _;
    let mut buffer = [0u8; 128];
    let mut used = 0usize;
    struct SliceWriter<'a>(&'a mut [u8], &'a mut usize);
    impl core::fmt::Write for SliceWriter<'_> {
        fn write_str(&mut self, text: &str) -> core::fmt::Result {
            let bytes = text.as_bytes();
            let room = self.0.len() - *self.1;
            let take = bytes.len().min(room);
            self.0[*self.1..*self.1 + take].copy_from_slice(&bytes[..take]);
            *self.1 += take;
            Ok(())
        }
    }
    let _ = write!(SliceWriter(&mut buffer, &mut used), "{info}");
    let _ = console::write_str("[PANIC] ");
    let _ = console::write_bytes(&buffer[..used]);
    let _ = console::write_str("\n");
    process::exit(-1)
}
