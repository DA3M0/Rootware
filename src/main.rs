#![no_std]
#![no_main]

mod gdt;
mod idt;
mod panic;
mod serial;
mod timer;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial::init();
    serial_println!("Before GDT init");
    gdt::init();
    serial_println!("Before IDT init");
    idt::init();
    serial_println!("Before timer init");
    timer::init();
    unsafe {
        core::arch::asm!("sti");
    }
    serial_println!("Interrupts enabled");
    serial_println!("After timer init");
    serial_println!("After IDT init");
    serial_println!("After GDT init");
    serial_println!("Rootware Microkernel");
    serial_println!("==============");
    serial_println!();
    serial_println!("Version: Alpha 3 ");
    serial_println!("License: Apache 2.0");
    serial_println!();
    serial_println!("Hello from Rootware!");
    serial_println!("GDT initialized");
    serial_println!("IDT initialized");
    serial_println!("Timer initialized");
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
