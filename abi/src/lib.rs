//! Rootware 1.0 frozen ABI (version 4).
//!
//! This crate is the single source of truth for every stable interface
//! between the Rootware kernel and user programs:
//!
//! - the `#[repr(C)]` [`ipc::Message`] layout (44 bytes, never resized),
//! - the `#[repr(C)]` [`rkm::RkmModule`] driver descriptor (32 bytes),
//! - standard capability kinds,
//! - the frozen syscall number table,
//! - error codes and their on-the-wire status encoding.
//!
//! Stability policy: after the 1.0 release this ABI only grows
//! additively. Existing syscall numbers, message layout and error
//! values keep their meaning for the whole 1.x series; breaking
//! changes require a new major version.

#![cfg_attr(not(test), no_std)]

pub mod capability;
pub mod error;
pub mod ipc;
pub mod rkm;
pub mod syscall;

pub use capability::{Capability, capability_kind};
pub use error::ErrorCode;
pub use ipc::{Message, RouteRule, message_type};
pub use rkm::{RkmModule, module_kind, module_state};

/// ABI version frozen for the Rootware 1.0 release.
pub const ABI_VERSION: u32 = 4;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn abi_version_is_frozen_at_four() {
        assert_eq!(ABI_VERSION, 4);
    }

    #[test]
    fn message_layout_is_frozen() {
        assert_eq!(size_of::<Message>(), 44);
        assert_eq!(size_of::<Capability>(), 4);
        assert_eq!(size_of::<RouteRule>(), 4);
    }

    #[test]
    fn error_statuses_round_trip() {
        for code in [
            ErrorCode::InvalidArgument,
            ErrorCode::PermissionDenied,
            ErrorCode::QueueFull,
            ErrorCode::QueueEmpty,
            ErrorCode::NotFound,
            ErrorCode::Unsupported,
            ErrorCode::Transport,
        ] {
            assert_eq!(ErrorCode::from_status(code.status()), Some(code));
        }
        assert_eq!(ErrorCode::from_status(0), None);
        assert_eq!(ErrorCode::from_status(-100), None);
    }

    #[test]
    fn syscall_numbers_are_frozen() {
        use crate::syscall::*;
        assert_eq!(SYS_IPC_SEND, 1);
        assert_eq!(SYS_IPC_RECEIVE, 2);
        assert_eq!(SYS_IPC_REPLY, 3);
        assert_eq!(SYS_VERSION, 4);
        assert_eq!(SYS_CONSOLE_WRITE, 5);
        assert_eq!(SYS_EXIT, 6);
        assert_eq!(SYS_SPAWN, 7);
        assert_eq!(SYS_CAP_REQUEST, 8);
        assert_eq!(SYS_MODULE_REGISTER, 9);
        assert_eq!(SYS_MODULE_LIST, 10);
    }

    #[test]
    fn capability_kinds_are_frozen() {
        use capability_kind::*;
        assert_eq!(IPC_SEND, 1);
        assert_eq!(FS_READ, 2);
        assert_eq!(FS_WRITE, 3);
    }
}
