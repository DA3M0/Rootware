# Echo service

The runnable echo service lives in `user/echo-service` and runs on the real
kernel: it blocks on `SYS_IPC_RECEIVE` for requests addressed to receiver 2
and answers each one as a `RESPONSE` back to the sender. `user/ipc-client`
performs the ABI handshake, requests its send capability and prints the
reply payload (`ping from client v1.0`).

The same flow can be exercised on Linux with the in-memory transport:

```rust
use librootware::capability::{capability_kind, Capability};
use librootware::ipc::{InMemoryTransport, Message, Transport, message_type, PAYLOAD_SIZE};

let mut ipc = InMemoryTransport::default();
let request = Message::new(
    1,
    2,
    message_type::REQUEST,
    Capability { id: capability_kind::IPC_SEND },
    [7; PAYLOAD_SIZE],
);
ipc.send(request)?;
let request = ipc.receive(2)?;
ipc.reply(&request, [8; PAYLOAD_SIZE])?;
```
