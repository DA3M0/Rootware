//! Native RKM driver: a resident counter device.
//!
//! Demonstrates the native driver path: register through the SDK,
//! learn the assigned pid (== IPC receiver id) from the returned
//! descriptor, then serve READ/WRITE requests over IPC forever.
//!
//! Driver payload convention (shared with the C compatibility shim,
//! see compat/linux/rkm.h): `payload[0]` is the op (OP_READ /
//! OP_WRITE), `payload[1]` the data length L ≤ 30, data in
//! `payload[2..2+L]`. Replies use `payload[1]` as status (0 ok /
//! 0xFF error). This device's data is an 8-byte little-endian counter
//! that increments on every read; a write resets it.

#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU64, Ordering};

use librootware::ipc::{PAYLOAD_SIZE, Transport, message_type};
use librootware::rkm::{self, OP_READ, OP_WRITE, module_kind};
use librootware::syscall::SyscallTransport;

static COUNTER: AtomicU64 = AtomicU64::new(0);

#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    let mut transport = SyscallTransport::new();

    // Registration tells the kernel who we are — and tells us which
    // pid, hence which IPC queue, the kernel assigned to this driver.
    let descriptor = match rkm::register("rkm-driver", "0.1.0", module_kind::NATIVE) {
        Ok(descriptor) => descriptor,
        Err(_) => return 1,
    };
    let receiver = descriptor.pid;

    loop {
        // Blocks inside the kernel until a request for this driver
        // arrives.
        let request = match transport.receive(receiver) {
            Ok(request) => request,
            Err(_) => return 2,
        };
        if request.message_type != message_type::DRIVER_REQUEST {
            continue;
        }
        let mut payload = [0u8; PAYLOAD_SIZE];
        payload[0] = request.payload[0];
        let len = (request.payload[1] as usize).min(30);
        match request.payload[0] {
            OP_READ => {
                let value = COUNTER.fetch_add(1, Ordering::Relaxed);
                let bytes = value.to_le_bytes();
                for (index, out) in payload[2..2 + len].iter_mut().enumerate() {
                    *out = bytes[index % 8];
                }
                payload[1] = 0;
            }
            OP_WRITE if len >= 8 => {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&request.payload[2..10]);
                COUNTER.store(u64::from_le_bytes(bytes), Ordering::Relaxed);
                payload[1] = 0;
            }
            // Unknown op or short write data: status byte reports it.
            _ => payload[1] = 0xFF,
        }
        if transport.reply(&request, payload).is_err() {
            return 3;
        }
    }
}
