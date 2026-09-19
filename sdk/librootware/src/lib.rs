//! Safe, testable wrappers around the Rootware userspace ABI.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

pub mod capability;
pub mod error;
pub mod ipc;
pub mod log;

pub use error::{Error, ErrorCode, Result};
pub use ipc::{Message, Transport};

/// ABI version implemented by this SDK.
pub const ABI_VERSION: u32 = 2;
