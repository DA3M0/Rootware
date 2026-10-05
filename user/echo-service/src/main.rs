//! Echo service: answers every request sent to receiver 2 with the same
//! payload as a response back to the sender. Runs forever.

#![no_std]
#![no_main]

use librootware::ipc::{Transport, message_type};
use librootware::syscall::SyscallTransport;

#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    let mut transport = SyscallTransport::new();
    loop {
        // Blocks inside the kernel until a message for this process arrives.
        let request = match transport.receive(2) {
            Ok(request) => request,
            Err(_) => return 1,
        };
        if request.message_type == message_type::REQUEST {
            if transport.reply(&request, request.payload).is_err() {
                return 2;
            }
        }
    }
}
