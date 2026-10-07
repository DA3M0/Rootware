//! Frozen IPC message wire format.

use crate::capability::Capability;

/// Payload bytes carried by every message.
pub const PAYLOAD_SIZE: usize = 32;
/// Receiver id used to reach every routed recipient of a broadcast.
pub const BROADCAST_RECEIVER: u16 = u16::MAX;

/// Standard message types understood by the kernel router.
pub mod message_type {
    pub const REQUEST: u16 = 1;
    pub const RESPONSE: u16 = 2;
    pub const EVENT: u16 = 3;
    pub const BROADCAST: u16 = 4;
    /// Direct RKM driver request (added 1.0.x for the driver framework).
    /// No route rule matches this type, so the router delivers it to
    /// the requested receiver — the driver's own pid.
    pub const DRIVER_REQUEST: u16 = 5;
}

/// One routing-table entry: messages of `message_type` are also
/// delivered to `receiver`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteRule {
    pub message_type: u16,
    pub receiver: u16,
}

/// The frozen 44-byte IPC message. Never resize or reorder these
/// fields; the kernel and every user binary exchange this struct
/// through raw pointers.
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
