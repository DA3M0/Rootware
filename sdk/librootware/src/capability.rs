pub use rootware_abi::capability::{capability_kind, Capability, INVALID_CAPABILITY};

use crate::error::{Error, ErrorCode, Result};

/// Requests a capability kind from the kernel grant table (ABI v4
/// SYS_CAP_REQUEST). The kernel grants it only when the boot policy
/// allows this process to hold it.
pub fn request_capability(kind: u32) -> Result<()> {
    if kind == INVALID_CAPABILITY {
        return Err(Error::new(ErrorCode::InvalidArgument));
    }
    unsafe { crate::syscall::invoke_raw(rootware_abi::syscall::SYS_CAP_REQUEST, kind as u64, 0) }
        .map(|_| ())
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
