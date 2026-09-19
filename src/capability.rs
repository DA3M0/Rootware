//! Beta 3 fixed-size capability registry.

pub const MAX_CAPABILITY_HOLDERS: usize = 8;
pub const INVALID_CAPABILITY: u32 = 0;
pub const IPC_SEND_CAPABILITY: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capability {
    pub id: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CapabilityGrant {
    owner: u16,
    capability: Capability,
}

static mut GRANTS: [Option<CapabilityGrant>; MAX_CAPABILITY_HOLDERS] =
    [None; MAX_CAPABILITY_HOLDERS];

pub fn grant(owner: u16, capability: Capability) -> Result<(), ()> {
    if capability.id == INVALID_CAPABILITY || owner == 0 {
        return Err(());
    }
    unsafe {
        let grants = &raw const GRANTS;
        for index in 0..MAX_CAPABILITY_HOLDERS {
            if (*grants)[index]
                .is_some_and(|grant| grant.owner == owner && grant.capability == capability)
            {
                return Ok(());
            }
        }
        let grants = &raw mut GRANTS;
        for index in 0..MAX_CAPABILITY_HOLDERS {
            if (*grants)[index].is_none() {
                (*grants)[index] = Some(CapabilityGrant { owner, capability });
                return Ok(());
            }
        }
    }
    Err(())
}

pub fn holds(owner: u16, capability: Capability) -> bool {
    if capability.id == INVALID_CAPABILITY {
        return false;
    }
    unsafe {
        let grants = &raw const GRANTS;
        (0..MAX_CAPABILITY_HOLDERS).any(|index| {
            (*grants)[index]
                .is_some_and(|grant| grant.owner == owner && grant.capability == capability)
        })
    }
}

pub fn init() {
    unsafe {
        GRANTS = [None; MAX_CAPABILITY_HOLDERS];
    }
    let _ = grant(
        1,
        Capability {
            id: IPC_SEND_CAPABILITY,
        },
    );
    crate::serial_println!("[CAP] capability registry initialized");
}

#[cfg(test)]
mod tests {
    use super::{Capability, IPC_SEND_CAPABILITY, grant, holds};

    #[test]
    fn capability_is_granted_to_owner() {
        let capability = Capability {
            id: IPC_SEND_CAPABILITY,
        };
        assert_eq!(capability.id, 1);
        assert!(grant(1, capability).is_ok());
        assert!(holds(1, capability));
        assert!(!holds(2, capability));
    }
}
