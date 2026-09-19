//! Minimal Ring 3 service loaded from a page and entered with iretq.

use core::ptr::write_volatile;

const USER_CODE: u64 = 0x4000_0000;
const USER_STACK: u64 = 0x4000_1000;
const CODE_PHYS: u64 = 0x0030_0000;
const STACK_PHYS: u64 = 0x0030_1000;

#[repr(C)]
pub struct UserServiceAbi {
    pub abi_version: u32,
    pub entry_point: u64,
    pub stack_pointer: u64,
}

static SERVICE: UserServiceAbi = UserServiceAbi {
    abi_version: 1,
    entry_point: USER_CODE,
    stack_pointer: USER_STACK + 4096 - 16,
};

pub fn init() -> ! {
    crate::serial_println!("[BETA] ABI v1 ready");
    crate::vmem::map_page(USER_CODE, CODE_PHYS, crate::vmem::USER | crate::vmem::WRITABLE)
        .expect("user code map failed");
    crate::vmem::map_page(USER_STACK, STACK_PHYS, crate::vmem::USER | crate::vmem::WRITABLE)
        .expect("user stack map failed");

    unsafe {
        // syscall; spin forever without privileged instructions.
        let code = [0x0f, 0x05, 0xeb, 0xfe];
        for (offset, byte) in code.iter().enumerate() {
            write_volatile((CODE_PHYS + offset as u64) as *mut u8, *byte);
        }
        crate::serial_println!("[SERVICE] entering Ring 3: Hello from userspace");
        enter_user(SERVICE.entry_point, SERVICE.stack_pointer)
    }
}

unsafe fn enter_user(entry: u64, stack: u64) -> ! {
    core::arch::asm!(
        "push {user_data}",
        "push {stack}",
        "pushfq",
        "push {user_code}",
        "push {entry}",
        "iretq",
        user_data = const 0x13u64,
        user_code = const 0x1bu64,
        entry = in(reg) entry,
        stack = in(reg) stack,
        options(noreturn)
    );
}
