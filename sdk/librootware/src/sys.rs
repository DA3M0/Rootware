//! Kernel introspection: the runtime ABI version handshake.
//!
//! A program should call [`check_abi_version`] once at startup and fail
//! fast when the running kernel implements a different ABI than the SDK
//! it was linked against.

use crate::error::{Error, Result};
use rootware_abi::syscall::SYS_VERSION;

/// The ABI version implemented by the running kernel.
pub fn kernel_abi_version() -> Result<u32> {
    let version = unsafe { crate::syscall::invoke_raw(SYS_VERSION, 0, 0)? };
    Ok(version as u32)
}

/// Verifies the running kernel implements an ABI this SDK understands:
/// anything inside the compatibility window `ABI_COMPAT_MIN ..
/// ABI_VERSION` (both re-exported at the crate root). Kernels older
/// than the window lack syscalls this SDK emits; kernels newer than it
/// may carry semantics this SDK does not know. ABI v4 kernels are
/// accepted but **deprecated** — new code targets v5, the only
/// supported interface.
pub fn check_abi_version() -> Result<()> {
    let kernel = kernel_abi_version()?;
    if kernel >= crate::ABI_COMPAT_MIN && kernel <= crate::ABI_VERSION {
        Ok(())
    } else {
        Err(Error::new(crate::error::ErrorCode::Unsupported))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn handshake_accepts_the_whole_compat_window() {
        let check = |kernel: u32| {
            kernel >= crate::ABI_COMPAT_MIN && kernel <= crate::ABI_VERSION
        };
        assert!(check(4), "deprecated v4 kernels stay accepted");
        assert!(check(5), "current v5 kernels accepted");
        assert!(!check(3), "pre-window kernels rejected");
        assert!(!check(6), "unknown newer kernels rejected");
    }
}
