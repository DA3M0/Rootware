use crate::capability::Capability;
use crate::error::{Error, ErrorCode, Result};

pub const PAYLOAD_SIZE: usize = 32;

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
    queue: std::collections::VecDeque<Message>,
    capacity: usize,
}

#[cfg(feature = "std")]
impl InMemoryTransport {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: std::collections::VecDeque::new(),
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
        if self.queue.len() == self.capacity {
            return Err(Error::new(ErrorCode::QueueFull));
        }
        self.queue.push_back(message);
        Ok(())
    }

    fn receive(&mut self, receiver: u16) -> Result<Message> {
        let index = self
            .queue
            .iter()
            .position(|m| m.receiver == receiver)
            .ok_or(Error::new(ErrorCode::QueueEmpty))?;
        self.queue
            .remove(index)
            .ok_or(Error::new(ErrorCode::QueueEmpty))
    }

    fn reply(&mut self, request: &Message, payload: [u8; PAYLOAD_SIZE]) -> Result<()> {
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
}
