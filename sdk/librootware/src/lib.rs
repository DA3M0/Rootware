//! Safe, testable wrappers around the Rootware userspace ABI.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std;

pub mod capability;
pub mod error;
pub mod ipc;
pub mod log;
pub mod service;
pub mod syscall;

pub use error::{Error, Result};
pub use ipc::{Message, Transport};
pub use rootware_abi::{ABI_VERSION, ErrorCode};
pub use syscall::SyscallTransport;
