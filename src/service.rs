//! Beta: stable handoff ABI for the first userspace service.

#[repr(C)]
pub struct UserServiceAbi {
    pub abi_version: u32,
    pub entry_point: u64,
    pub stack_pointer: u64,
}

static SERVICE: UserServiceAbi = UserServiceAbi {
    abi_version: 1,
    entry_point: 0,
    stack_pointer: 0,
};

extern "C" fn user_service() {
    crate::serial_println!("[SERVICE] first user service loaded");
}

pub fn init() {
    let _ = SERVICE.abi_version;
    crate::serial_println!("[BETA] ABI v1 ready");
    user_service();
}
