use crate::serial_println;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[PANIC] {}", info);
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
