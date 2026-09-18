#![no_std]
#![no_main]

mod gdt;
mod idt;
mod memory;
mod panic;
mod serial;
mod timer;

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

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
