use crate::error::Result;
#[cfg(feature = "std")]
use crate::error::{Error, ErrorCode};

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capability {
    pub id: u32,
}

pub const INVALID_CAPABILITY: u32 = 0;

/// Standard capability namespaces shared by the kernel and SDK.
pub mod capability_kind {
    /// Permission to send IPC messages.
    pub const IPC_SEND: u32 = 1;
    /// Permission to read filesystem objects.
    pub const FS_READ: u32 = 2;
    /// Permission to write filesystem objects.
    pub const FS_WRITE: u32 = 3;
}

pub trait CapabilityProvider {
    fn request(&mut self, capability: Capability) -> Result<()>;
    fn check(&self, capability: Capability) -> bool;
}

#[cfg(feature = "std")]
#[derive(Default)]
pub struct InMemoryCapabilities {
    granted: std::collections::BTreeSet<u32>,
}

#[cfg(feature = "std")]
impl CapabilityProvider for InMemoryCapabilities {
    fn request(&mut self, capability: Capability) -> Result<()> {
        if capability.id == INVALID_CAPABILITY {
            return Err(Error::new(ErrorCode::InvalidArgument));
        }
        self.granted.insert(capability.id);
        Ok(())
    }
    fn check(&self, capability: Capability) -> bool {
        self.granted.contains(&capability.id)
    }
}
