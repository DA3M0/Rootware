use crate::capability::Capability;
use crate::error::Result;
#[cfg(feature = "std")]
use crate::error::{Error, ErrorCode};

pub const PAYLOAD_SIZE: usize = 32;
pub const BROADCAST_RECEIVER: u16 = u16::MAX;
pub const ABI_VERSION: u32 = 3;

pub mod message_type {
    pub const REQUEST: u16 = 1;
    pub const RESPONSE: u16 = 2;
    pub const EVENT: u16 = 3;
    pub const BROADCAST: u16 = 4;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteRule {
    pub message_type: u16,
    pub receiver: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Message {
    pub sender: u16,
    pub receiver: u16,
    pub message_type: u16,
    pub capability: Capability,
    pub payload: [u8; PAYLOAD_SIZE],
}

impl Message {
    pub const fn new(
        sender: u16,
        receiver: u16,
        message_type: u16,
        capability: Capability,
        payload: [u8; PAYLOAD_SIZE],
    ) -> Self {
        Self {
            sender,
            receiver,
            message_type,
            capability,
            payload,
        }
    }
}

/// Kernel-facing transport. A syscall-backed implementation can be supplied later.
pub trait Transport {
    fn send(&mut self, message: Message) -> Result<()>;
    fn receive(&mut self, receiver: u16) -> Result<Message>;
    fn reply(&mut self, request: &Message, payload: [u8; PAYLOAD_SIZE]) -> Result<()>;
}

#[cfg(feature = "std")]
pub struct InMemoryTransport {
    queues: std::collections::HashMap<u16, std::collections::VecDeque<Message>>,
    queued: usize,
    capacity: usize,
}

#[cfg(feature = "std")]
impl InMemoryTransport {
    pub fn new(capacity: usize) -> Self {
        Self {
            queues: std::collections::HashMap::new(),
            queued: 0,
            capacity,
        }
    }
}

#[cfg(feature = "std")]
impl Default for InMemoryTransport {
    fn default() -> Self {
        Self::new(8)
    }
}

#[cfg(feature = "std")]
impl Transport for InMemoryTransport {
    fn send(&mut self, message: Message) -> Result<()> {
        if message.sender == 0 || message.receiver == 0 || message.capability.id == 0 {
            return Err(Error::new(ErrorCode::PermissionDenied));
        }
        if self.queued == self.capacity {
            return Err(Error::new(ErrorCode::QueueFull));
        }
        self.queues
            .entry(message.receiver)
            .or_default()
            .push_back(message);
        self.queued += 1;
        Ok(())
    }

    fn receive(&mut self, receiver: u16) -> Result<Message> {
        let queue = self
            .queues
            .get_mut(&receiver)
            .ok_or(Error::new(ErrorCode::QueueEmpty))?;
        let message = queue
            .pop_front()
            .ok_or(Error::new(ErrorCode::QueueEmpty))?;
        self.queued -= 1;
        if queue.is_empty() {
            self.queues.remove(&receiver);
        }
        Ok(message)
    }

    fn reply(&mut self, request: &Message, payload: [u8; PAYLOAD_SIZE]) -> Result<()> {
        if request.receiver == 0 || request.sender == 0 {
            return Err(Error::new(ErrorCode::InvalidArgument));
        }
        self.send(Message::new(
            request.receiver,
            request.sender,
            request.message_type,
            request.capability,
            payload,
        ))
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    #[test]
    fn sends_receives_and_replies() {
        let mut transport = InMemoryTransport::default();
        let request = Message::new(1, 2, 7, Capability { id: 1 }, [1; PAYLOAD_SIZE]);
        transport.send(request).unwrap();
        assert_eq!(transport.receive(2).unwrap(), request);
        transport.reply(&request, [2; PAYLOAD_SIZE]).unwrap();
        assert_eq!(transport.receive(1).unwrap().payload, [2; PAYLOAD_SIZE]);
    }

    #[test]
    fn standard_message_types_are_stable() {
        assert_eq!(message_type::REQUEST, 1);
        assert_eq!(message_type::BROADCAST, 4);
        assert_eq!(BROADCAST_RECEIVER, u16::MAX);
    }

    #[test]
    fn receiver_queues_are_isolated() {
        let mut transport = InMemoryTransport::new(2);
        let first = Message::new(1, 2, message_type::EVENT, Capability { id: 1 }, [2; PAYLOAD_SIZE]);
        let second = Message::new(1, 3, message_type::EVENT, Capability { id: 1 }, [3; PAYLOAD_SIZE]);
        transport.send(first).unwrap();
        transport.send(second).unwrap();
        assert_eq!(transport.receive(3).unwrap(), second);
        assert_eq!(transport.receive(2).unwrap(), first);
    }

    #[test]
    fn reply_rejects_invalid_routes() {
        let mut transport = InMemoryTransport::default();
        let request = Message::new(0, 2, message_type::REQUEST, Capability { id: 1 }, [0; PAYLOAD_SIZE]);
        assert_eq!(
            transport.reply(&request, [0; PAYLOAD_SIZE]),
            Err(Error::new(ErrorCode::InvalidArgument))
        );
    }

    #[test]
    fn throughput_and_no_loss_under_pressure() {
        let mut transport = InMemoryTransport::new(256);
        let total = 10_000;
        for index in 0..total {
            let mut payload = [0; PAYLOAD_SIZE];
            payload[..8].copy_from_slice(&(index as u64).to_le_bytes());
            transport
                .send(Message::new(
                    1,
                    2,
                    message_type::EVENT,
                    Capability { id: 1 },
                    payload,
                ))
                .unwrap_or_else(|_| panic!("queue rejected message {index}"));
            let received = transport.receive(2).unwrap();
            assert_eq!(
                u64::from_le_bytes(received.payload[..8].try_into().unwrap()),
                index as u64
            );
        }
    }

    #[test]
    fn abi_version_and_message_size_are_frozen() {
        assert_eq!(ABI_VERSION, 3);
        assert_eq!(core::mem::size_of::<Message>(), 44);
    }
}
