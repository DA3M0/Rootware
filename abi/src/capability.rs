//! Stable capability tokens and standard kinds.

/// A capability token. The id is only meaningful together with the
/// kernel's grant table; `0` is permanently invalid.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capability {
    pub id: u32,
}

/// The invalid capability id; never grantable, never held.
pub const INVALID_CAPABILITY: u32 = 0;

/// Standard capability namespaces shared by the kernel and SDK.
pub mod capability_kind {
    /// Permission to send IPC messages.
    pub const IPC_SEND: u32 = 1;
    /// Permission to read filesystem objects. Reserved until the
    /// filesystem layer ships; the kernel never grants it in 1.0.
    pub const FS_READ: u32 = 2;
    /// Permission to write filesystem objects. Reserved as above.
    pub const FS_WRITE: u32 = 3;
}
