# Echo service

```rust
use librootware::ipc::{InMemoryTransport, Message, Transport};

let mut ipc = InMemoryTransport::default();
let request = Message::new(1, 2, 1, [7; 32]);
ipc.send(request)?;
let request = ipc.receive(2)?;
ipc.reply(&request, [8; 32])?;
```

This example demonstrates the same send/receive/reply flow used by a service
running under the Linux simulator.
