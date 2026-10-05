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

/// Verifies the running kernel matches this SDK's ABI version.
pub fn check_abi_version() -> Result<()> {
    let kernel = kernel_abi_version()?;
    if kernel == crate::ABI_VERSION {
        Ok(())
    } else {
        Err(Error::new(crate::error::ErrorCode::Unsupported))
    }
}
