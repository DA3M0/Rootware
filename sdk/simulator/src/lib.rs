//! Linux-hosted simulator primitives for SDK integration tests.

use librootware::capability::Capability;
use librootware::ipc::{InMemoryTransport, Message, Transport};

pub struct Simulator {
    pub ipc: InMemoryTransport,
}

impl Default for Simulator {
    fn default() -> Self {
        Self {
            ipc: InMemoryTransport::default(),
        }
    }
}

impl Simulator {
    pub fn send(&mut self, message: Message) -> librootware::Result<()> {
        self.ipc.send(message)
    }
    pub fn receive(&mut self, receiver: u16) -> librootware::Result<Message> {
        self.ipc.receive(receiver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_round_trip() {
        let mut simulator = Simulator::default();
        simulator
            .send(Message::new(1, 2, 1, Capability { id: 1 }, [42; 32]))
            .unwrap();
        assert_eq!(simulator.receive(2).unwrap().payload[0], 42);
    }
}
