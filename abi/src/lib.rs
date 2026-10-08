//! Rootware 2.0 frozen ABI (version 5; version 4 deprecated).
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
//! Stability policy: since the 1.0 release this ABI only grows
//! additively. Existing syscall numbers, message layout and error
//! values keep their meaning for the whole 2.x series; breaking
//! changes require a new major version. ABI v4 is **deprecated** as of
//! 2.0: v5 is a pure superset of v4 (syscall 11 appended; numbers 1–10,
//! the `Message` layout and every error code untouched), the kernel
//! interface speaks v5 exclusively, and [`ABI_COMPAT_MIN`] keeps the
//! runtime handshake accepting pre-2.0 v4 kernels only so existing
//! binaries keep running. New code must target v5.

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

/// ABI version. 4 was frozen for the 1.0 release; 5 adds
/// `SYS_CONSOLE_READ` (console input) as an append-only extension and
/// is the only supported interface as of the 2.0 release.
pub const ABI_VERSION: u32 = 5;

/// Oldest ABI version the runtime handshake still accepts. ABI v4 is
/// **deprecated** since 2.0; this window exists purely so binaries
/// built before the 2.0 release keep running. New code and every
/// in-tree component target [`ABI_VERSION`] (v5).
pub const ABI_COMPAT_MIN: u32 = 4;

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn abi_version_matches_five() {
        assert_eq!(ABI_VERSION, 5);
    }

    #[test]
    fn compat_window_accepts_deprecated_v4() {
        assert_eq!(ABI_COMPAT_MIN, 4);
        assert!(ABI_COMPAT_MIN < ABI_VERSION);
        // v4 binaries keep passing the handshake; v5 is the target.
        assert!((ABI_COMPAT_MIN..=ABI_VERSION).contains(&4));
        assert!((ABI_COMPAT_MIN..=ABI_VERSION).contains(&ABI_VERSION));
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
        assert_eq!(SYS_CONSOLE_READ, 11);
    }

    #[test]
    fn capability_kinds_are_frozen() {
        use capability_kind::*;
        assert_eq!(IPC_SEND, 1);
        assert_eq!(FS_READ, 2);
        assert_eq!(FS_WRITE, 3);
    }
}
