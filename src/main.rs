#![no_std]
#![no_main]

mod gdt;
mod panic;
mod serial;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial::init();
    serial_println!("Before GDT init");
    gdt::init();
    serial_println!("After GDT init");
    serial_println!("Rootware 0.0.2");
    serial_println!("==============");
    serial_println!();
    serial_println!("Kernel:  Rootware Microkernel");
    serial_println!("Version: 0.0.2 (no-lib, 2024)");
    serial_println!("License: Apache 2.0");
    serial_println!();
    serial_println!("Hello from Rootware!");
    serial_println!("GDT initialized");
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
