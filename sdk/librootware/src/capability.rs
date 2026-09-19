use crate::error::{Error, ErrorCode, Result};

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capability {
    pub id: u32,
}

pub const INVALID_CAPABILITY: u32 = 0;

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
