//! Rootware Kernel Module (RKM) wire format.
//!
//! An RKM module is a user-space driver process. It announces itself to
//! the kernel through `SYS_MODULE_REGISTER` and is enumerated through
//! `SYS_MODULE_LIST`. Both native Rust drivers (librootware SDK) and
//! Linux-compatible C drivers (compat shim) share this format.
//!
//! The descriptor is a frozen 32-byte layout passed through raw
//! pointers: the caller fills `name`, `version` and `kind`, and the
//! kernel fills `pid` and `state` before writing the descriptor back.

/// Maximum module name length in bytes (NUL padding included).
pub const MAX_MODULE_NAME: usize = 16;
/// Maximum module version string length in bytes (NUL padding included).
pub const MAX_MODULE_VERSION: usize = 8;

/// Standard module kinds.
pub mod module_kind {
    /// Rust driver built against the librootware SDK.
    pub const NATIVE: u8 = 1;
    /// C driver compiled through the Linux compatibility shim.
    pub const LINUX: u8 = 2;
}

/// Module lifecycle states tracked by the kernel.
pub mod module_state {
    /// Registered and owned by a live process.
    pub const ACTIVE: u8 = 1;
    /// The owning process exited; the entry is retained for inspection
    /// until its slot is reused by a new registration.
    pub const STOPPED: u8 = 2;
}

/// First payload byte of a driver request: read from the device.
pub const OP_READ: u8 = 1;
/// First payload byte of a driver request: write to the device.
pub const OP_WRITE: u8 = 2;

/// The frozen 32-byte module descriptor. Never resize or reorder these
/// fields; the kernel and every driver exchange this struct through raw
/// pointers on both the Rust and the C side.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RkmModule {
    pub name: [u8; MAX_MODULE_NAME],
    pub version: [u8; MAX_MODULE_VERSION],
    /// Process id of the owning driver; filled in by the kernel.
    pub pid: u16,
    /// Lifecycle state; filled in by the kernel.
    pub state: u8,
    /// One of [`module_kind`]; validated by the kernel.
    pub kind: u8,
    pub _pad: [u8; 4],
}

impl RkmModule {
    /// Builds a registration request. `name` and `version` are truncated
    /// to the frozen limits; `pid` and `state` start at zero and are
    /// filled in by the kernel on a successful `SYS_MODULE_REGISTER`.
    pub fn new(name: &str, version: &str, kind: u8) -> Self {
        let mut descriptor = Self {
            name: [0; MAX_MODULE_NAME],
            version: [0; MAX_MODULE_VERSION],
            pid: 0,
            state: 0,
            kind,
            _pad: [0; 4],
        };
        let name = name.as_bytes();
        let take_name = name.len().min(MAX_MODULE_NAME);
        descriptor.name[..take_name].copy_from_slice(&name[..take_name]);
        let version = version.as_bytes();
        let take_version = version.len().min(MAX_MODULE_VERSION);
        descriptor.version[..take_version].copy_from_slice(&version[..take_version]);
        descriptor
    }

    /// The stored name up to its NUL padding, if it is valid UTF-8.
    pub fn name_str(&self) -> Option<&str> {
        let end = self.name.iter().position(|byte| *byte == 0).unwrap_or(MAX_MODULE_NAME);
        core::str::from_utf8(&self.name[..end]).ok()
    }

    /// The stored version up to its NUL padding, if it is valid UTF-8.
    pub fn version_str(&self) -> Option<&str> {
        let end = self.version.iter().position(|byte| *byte == 0).unwrap_or(MAX_MODULE_VERSION);
        core::str::from_utf8(&self.version[..end]).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::size_of;

    #[test]
    fn descriptor_layout_is_frozen() {
        assert_eq!(size_of::<RkmModule>(), 32);
    }

    #[test]
    fn new_truncates_and_zero_pads() {
        let descriptor = RkmModule::new("zero-driver", "0.1.0", module_kind::LINUX);
        assert_eq!(descriptor.name_str(), Some("zero-driver"));
        assert_eq!(descriptor.version_str(), Some("0.1.0"));
        assert_eq!(descriptor.kind, module_kind::LINUX);
        assert_eq!(descriptor.pid, 0);
        assert_eq!(descriptor.state, 0);

        let long = RkmModule::new("0123456789abcdefX", "9.9.9-long", module_kind::NATIVE);
        assert_eq!(long.name.len(), MAX_MODULE_NAME);
        assert_eq!(&long.name_str().unwrap()[..16], "0123456789abcdef");
        assert_eq!(long.version_str(), Some("9.9.9-lo"));
    }

    #[test]
    fn kinds_and_states_are_frozen() {
        use module_kind::*;
        use module_state::*;
        assert_eq!(NATIVE, 1);
        assert_eq!(LINUX, 2);
        assert_eq!(ACTIVE, 1);
        assert_eq!(STOPPED, 2);
        assert_eq!(OP_READ, 1);
        assert_eq!(OP_WRITE, 2);
    }
}
