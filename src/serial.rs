use core::arch::asm;
use core::fmt;

const COM1: u16 = 0x3F8;

pub struct SerialPort;

impl SerialPort {
    pub const fn new() -> Self {
        SerialPort
    }

    pub fn init(&self) {
        unsafe {
            outb(COM1 + 1, 0x00);
            outb(COM1 + 3, 0x80);
            outb(COM1 + 0, 0x03);
            outb(COM1 + 1, 0x00);
            outb(COM1 + 3, 0x03);
            outb(COM1 + 2, 0xC7);
            outb(COM1 + 4, 0x0B);
        }
    }

    pub fn write_byte(&self, byte: u8) {
        unsafe {
            while (inb(COM1 + 5) & 0x20) == 0 {}
            outb(COM1, byte);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack));
    }
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack));
    }
    value
}

static mut SERIAL: SerialPort = SerialPort::new();

pub fn init() {
    unsafe {
        (*&raw mut SERIAL).init();
    }
}

pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    unsafe {
        let _ = (*&raw mut SERIAL).write_fmt(args);
    }
}

pub fn write_byte(byte: u8) {
    unsafe {
        (*&raw mut SERIAL).write_byte(byte);
    }
}

pub fn write_str(s: &str) {
    use core::fmt::Write;
    unsafe {
        let _ = (*&raw mut SERIAL).write_str(s);
    }
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
