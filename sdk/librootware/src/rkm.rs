//! Rootware Kernel Module (RKM) registration and enumeration.
//!
//! A driver process registers itself once at startup and stays resident
//! to answer IPC requests. Registration returns the descriptor as
//! stored by the kernel, whose `pid` field tells the driver which
//! receiver id it owns.

pub use rootware_abi::rkm::{OP_READ, OP_WRITE, module_kind, module_state};

use crate::error::{Error, ErrorCode, Result};
use crate::ipc::Message;
use rootware_abi::rkm::{RkmModule, MAX_MODULE_NAME, MAX_MODULE_VERSION};

/// A Rootware user-space driver.
pub trait Driver {
    /// Stable module name shown in the kernel registry.
    fn name(&self) -> &str;

    /// Module version string.
    fn version(&self) -> &str;

    /// Module kind; native SDK drivers default to [`module_kind::NATIVE`].
    fn kind(&self) -> u8 {
        module_kind::NATIVE
    }

    /// Prepares driver state before it accepts requests.
    fn init(&mut self) -> Result<()>;

    /// Handles one request and optionally returns the response message.
    fn handle(&mut self, request: &Message) -> Result<Option<Message>>;

    /// Releases driver-owned state.
    fn stop(&mut self) {}
}

/// Registers the calling process as an RKM module (ABI v4
/// SYS_MODULE_REGISTER) and returns the descriptor as stored by the
/// kernel, with `pid` and `state` filled in.
pub fn register(name: &str, version: &str, kind: u8) -> Result<RkmModule> {
    if kind != module_kind::NATIVE && kind != module_kind::LINUX {
        return Err(Error::new(ErrorCode::InvalidArgument));
    }
    if name.is_empty() || name.len() > MAX_MODULE_NAME {
        return Err(Error::new(ErrorCode::InvalidArgument));
    }
    if version.len() > MAX_MODULE_VERSION {
        return Err(Error::new(ErrorCode::InvalidArgument));
    }
    let mut descriptor = RkmModule::new(name, version, kind);
    unsafe {
        crate::syscall::invoke_raw(
            rootware_abi::syscall::SYS_MODULE_REGISTER,
            &raw mut descriptor as u64,
            0,
        )?;
    }
    Ok(descriptor)
}

/// Copies the kernel's module registry into `buffer` (ABI v4
/// SYS_MODULE_LIST) and returns the total number of stored modules,
/// which may exceed `buffer.len()`; call again with a bigger buffer to
/// see the rest.
pub fn list(buffer: &mut [RkmModule]) -> Result<usize> {
    if buffer.is_empty() {
        return Err(Error::new(ErrorCode::InvalidArgument));
    }
    let total = unsafe {
        crate::syscall::invoke_raw(
            rootware_abi::syscall::SYS_MODULE_LIST,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
        )?
    };
    Ok(total as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_registrations_fail_before_reaching_the_kernel() {
        assert_eq!(
            register("", "1.0", module_kind::NATIVE).unwrap_err().code(),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            register("0123456789abcdef0", "1.0", module_kind::NATIVE)
                .unwrap_err()
                .code(),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            register("name", "1.0.0-too-long", module_kind::NATIVE)
                .unwrap_err()
                .code(),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            register("name", "1.0", 9).unwrap_err().code(),
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn kernel_calls_report_unsupported_off_rootware() {
        // On the host there is no kernel behind the syscall ABI.
        assert_eq!(
            register("name", "1.0", module_kind::NATIVE).unwrap_err().code(),
            ErrorCode::Unsupported
        );
        assert_eq!(
            list(&mut []).unwrap_err().code(),
            ErrorCode::InvalidArgument
        );
        let mut buffer = [RkmModule::new("x", "1.0", module_kind::NATIVE)];
        assert_eq!(list(&mut buffer).unwrap_err().code(), ErrorCode::Unsupported);
    }

    #[test]
    fn descriptor_round_trips_through_the_frozen_layout() {
        let descriptor = RkmModule::new("zero-driver", "0.1.0", module_kind::LINUX);
        assert_eq!(descriptor.name_str(), Some("zero-driver"));
        assert_eq!(descriptor.version_str(), Some("0.1.0"));
    }
}
