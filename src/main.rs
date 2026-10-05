#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

// Kernel entry point, called from `boot.asm` with the Multiboot2
// information address in the first argument. Under host tests the
// hardware bring-up is skipped and only the kernel logic is compiled.

extern crate alloc;

mod gdt;
mod capability;
mod audit;
mod elf;
mod heap;
mod idt;
mod memory;
#[cfg(not(test))]
mod panic;
mod process;
mod selftest;
mod serial;
mod syscall;
mod timer;
mod vmem;
mod ipc;

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn rust_start(mb_info: u64) -> ! {
    serial::init();

    // 只取低 32 位
    let mb_info_low = mb_info & 0xFFFFFFFF;

    serial::write_str("mb_info low: 0x");
    for i in (0..8).rev() {
        let nibble = ((mb_info_low >> (i * 4)) & 0xF) as u8;
        let c = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + nibble - 10
        };
        serial::write_byte(c);
    }
    serial::write_str("\n");

    memory::init(mb_info_low);
    heap::init();
    vmem::init();
    vmem::selftest();
    capability::init();
    audit::init();
    ipc::init();
    gdt::init();
    idt::init();
    timer::init();
    selftest::run_all();
    // From here on the APIC timer fires; all handlers are installed and
    // the TSS provides kernel stacks for interrupts from Ring 3.
    unsafe {
        core::arch::asm!("sti", options(nostack));
    }
    crate::serial_println!("[BOOT] interrupts enabled");
    process::init_and_run();
}
