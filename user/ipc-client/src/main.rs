//! IPC client: performs the ABI version handshake, requests its IPC send
//! capability, sends a request to the echo service and prints the reply.

#![no_std]
#![no_main]

use librootware::capability::{Capability, capability_kind, request_capability};
use librootware::console;
use librootware::ipc::{Message, PAYLOAD_SIZE, Transport, message_type};
use librootware::syscall::SyscallTransport;
use librootware::sys;

#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    match sys::kernel_abi_version() {
        Ok(version) if version == librootware::ABI_VERSION => {
            let _ = console::write_args(format_args!(
                "client: kernel ABI v{} handshake ok\n",
                version
            ));
        }
        Ok(version) => {
            let _ = console::write_args(format_args!(
                "client: kernel ABI v{} but SDK expects v{}\n",
                version,
                librootware::ABI_VERSION
            ));
            return 1;
        }
        Err(_) => {
            let _ = console::write_str("client: version handshake syscall failed\n");
            return 1;
        }
    }

    if request_capability(capability_kind::IPC_SEND).is_err() {
        let _ = console::write_str("client: IPC_SEND capability denied\n");
        return 1;
    }

    let mut payload = [0u8; PAYLOAD_SIZE];
    payload[..21].copy_from_slice(b"ping from client v1.0");

    let mut transport = SyscallTransport::new();
    let request = Message::new(
        1,
        2,
        message_type::REQUEST,
        Capability {
            id: capability_kind::IPC_SEND,
        },
        payload,
    );

    if let Err(_) = transport.send(request) {
        let _ = console::write_str("client: send failed\n");
        return 1;
    }

    match transport.receive(1) {
        Ok(response) => {
            let _ = console::write_str("client: echo reply = ");
            let len = response
                .payload
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(PAYLOAD_SIZE);
            let _ = console::write_bytes(&response.payload[..len]);
            let _ = console::write_str("\n");
            0
        }
        Err(_) => {
            let _ = console::write_str("client: receive failed\n");
            1
        }
    }
}
