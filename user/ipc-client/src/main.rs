//! IPC client: performs the ABI version handshake, requests its IPC send
//! capability, exchanges an echo round trip with the echo service, then
//! enumerates the RKM module registry and reads 8 zero bytes from the
//! Linux-compatible zero-driver through the kernel IPC path.

#![no_std]
#![no_main]

use librootware::capability::{Capability, capability_kind, request_capability};
use librootware::console;
use librootware::ipc::{Message, PAYLOAD_SIZE, Transport, message_type};
use librootware::rkm;
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

    if transport.send(request).is_err() {
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
        }
        Err(_) => {
            let _ = console::write_str("client: receive failed\n");
            return 1;
        }
    }

    // RKM end-to-end: enumerate the module registry, find the C
    // zero-driver, then read through it. This exercises the whole
    // chain: SDK list syscall -> kernel registry -> DRIVER_REQUEST
    // routing -> C driver ops -> reply back to this process.
    let mut modules = [librootware::RkmModule::new("", "", 0); 8];
    let total = match rkm::list(&mut modules) {
        Ok(total) => total,
        Err(_) => {
            let _ = console::write_str("client: module list syscall failed\n");
            return 1;
        }
    };
    let _ = console::write_args(format_args!("client: {} RKM modules registered\n", total));

    let zero = modules[..total.min(modules.len())]
        .iter()
        .find(|module| module.name_str() == Some("zero-driver"));
    let Some(zero) = zero else {
        let _ = console::write_str("client: zero-driver not in the registry\n");
        return 1;
    };

    let mut payload = [0u8; PAYLOAD_SIZE];
    payload[0] = rkm::OP_READ;
    payload[1] = 8;
    let request = Message::new(
        1,
        zero.pid,
        message_type::DRIVER_REQUEST,
        Capability {
            id: capability_kind::IPC_SEND,
        },
        payload,
    );
    if transport.send(request).is_err() {
        let _ = console::write_str("client: driver request denied\n");
        return 1;
    }
    match transport.receive(1) {
        Ok(response) => {
            let zeros = response.payload[0] == rkm::OP_READ
                && response.payload[1] == 0
                && response.payload[2..10].iter().all(|&byte| byte == 0);
            if zeros {
                let _ = console::write_str("client: zero-driver read ok: 8 zero bytes\n");
                0
            } else {
                let _ = console::write_str("client: zero-driver read mismatch\n");
                1
            }
        }
        Err(_) => {
            let _ = console::write_str("client: driver reply missing\n");
            1
        }
    }
}
