//! Smallest Rootware user program: print to the console and exit.

#![no_std]
#![no_main]

use librootware::console;

#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    let _ = console::write_str("hello from Rootware userspace\n");
    0
}
