//! IPC: per-receiver message queues with configurable rules and
//! capabilities.
//!
//! Every receiver owns a bounded queue, so a slow consumer can no longer
//! block delivery to another receiver (the Beta single-ring limitation).
//! Delivering a message wakes one process blocked on that receiver.

use alloc::collections::{BTreeMap, vec_deque::VecDeque};

pub use rootware_abi::ipc::{message_type, Message, RouteRule, BROADCAST_RECEIVER, PAYLOAD_SIZE};

use rootware_abi::ErrorCode;

use crate::capability::{self, Capability};
pub const QUEUE_CAPACITY: usize = 16;
pub const MAX_PERMISSION_RULES: usize = 8;
pub const MAX_ROUTE_RULES: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PermissionRule {
    pub sender: u16,
    pub receiver: u16,
    pub capability: Capability,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PermissionConfig {
    pub rules: [PermissionRule; MAX_PERMISSION_RULES],
    pub count: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RouteConfig {
    pub rules: [RouteRule; MAX_ROUTE_RULES],
    pub count: u16,
}

impl RouteConfig {
    pub const fn with_rules(rules: &[RouteRule]) -> Self {
        let mut config = Self {
            rules: [RouteRule {
                message_type: 0,
                receiver: 0,
            }; MAX_ROUTE_RULES],
            count: 0,
        };
        let mut index = 0;
        while index < rules.len() && index < MAX_ROUTE_RULES {
            config.rules[index] = rules[index];
            index += 1;
        }
        config.count = index as u16;
        config
    }

    fn targets(
        &self,
        message_type: u16,
        requested: u16,
        targets: &mut [u16; MAX_ROUTE_RULES],
    ) -> usize {
        let count = core::cmp::min(self.count as usize, MAX_ROUTE_RULES);
        let mut found = 0;
        for rule in &self.rules[..count] {
            if rule.message_type == message_type && found < MAX_ROUTE_RULES {
                targets[found] = rule.receiver;
                found += 1;
            }
        }
        if found == 0 && requested != BROADCAST_RECEIVER {
            targets[0] = requested;
            1
        } else {
            found
        }
    }
}

impl PermissionConfig {
    pub const fn empty() -> Self {
        Self {
            rules: [PermissionRule {
                sender: 0,
                receiver: 0,
                capability: Capability { id: 0 },
            }; MAX_PERMISSION_RULES],
            count: 0,
        }
    }

    pub const fn with_rules(rules: &[PermissionRule]) -> Self {
        let mut config = Self::empty();
        let mut index = 0;
        while index < rules.len() && index < MAX_PERMISSION_RULES {
            config.rules[index] = rules[index];
            index += 1;
        }
        config.count = index as u16;
        config
    }

    pub fn allows(&self, sender: u16, receiver: u16, capability: Capability) -> bool {
        let count = core::cmp::min(self.count as usize, MAX_PERMISSION_RULES);
        self.rules[..count].iter().any(|rule| {
            rule.sender == sender && rule.receiver == receiver && rule.capability == capability
        })
    }
}

const DEFAULT_PERMISSIONS: PermissionConfig = PermissionConfig::with_rules(&[
    PermissionRule {
        sender: 1,
        receiver: 2,
        capability: Capability {
            id: capability::IPC_SEND_CAPABILITY,
        },
    },
    PermissionRule {
        sender: 1,
        receiver: 3,
        capability: Capability {
            id: capability::IPC_SEND_CAPABILITY,
        },
    },
    // Replies: the echo service answers the client.
    PermissionRule {
        sender: 2,
        receiver: 1,
        capability: Capability {
            id: capability::IPC_SEND_CAPABILITY,
        },
    },
]);
const DEFAULT_ROUTES: RouteConfig = RouteConfig::with_rules(&[
    RouteRule {
        message_type: message_type::REQUEST,
        receiver: 2,
    },
    RouteRule {
        message_type: message_type::RESPONSE,
        receiver: 1,
    },
    RouteRule {
        message_type: message_type::EVENT,
        receiver: 2,
    },
    RouteRule {
        message_type: message_type::BROADCAST,
        receiver: 2,
    },
    RouteRule {
        message_type: message_type::BROADCAST,
        receiver: 3,
    },
]);

static mut QUEUES: Option<BTreeMap<u16, VecDeque<Message>>> = None;
static mut PERMISSIONS: PermissionConfig = DEFAULT_PERMISSIONS;
static mut ROUTES: RouteConfig = DEFAULT_ROUTES;

unsafe fn queues() -> &'static mut BTreeMap<u16, VecDeque<Message>> {
    unsafe {
        let slot = &raw mut QUEUES;
        if (*slot).is_none() {
            (*slot) = Some(BTreeMap::new());
        }
        (*slot).as_mut().expect("ipc queues initialized")
    }
}

fn allowed(message: &Message, receiver: u16) -> bool {
    unsafe {
        let config = &raw const PERMISSIONS;
        (&*config).allows(message.sender, receiver, message.capability)
    }
}

fn capability_allowed(message: &Message) -> bool {
    capability::holds(message.sender, message.capability)
}

pub fn load_permissions(config: &PermissionConfig) {
    unsafe {
        (&raw mut PERMISSIONS).write(*config);
    }
    crate::serial_println!("[IPC] permission table loaded");
}

pub fn load_routes(config: &RouteConfig) {
    unsafe {
        (&raw mut ROUTES).write(*config);
    }
    crate::serial_println!("[IPC] route table loaded");
}

fn audit(message: &Message, receiver: u16, result: u16) {
    crate::audit::record(crate::audit::AuditEntry {
        timestamp: crate::timer::ticks(),
        sender: message.sender,
        receiver,
        message_type: message.message_type,
        result,
        capability: message.capability.id,
    });
}

pub fn send(message: Message) -> Result<(), ErrorCode> {
    let mut targets = [0; MAX_ROUTE_RULES];
    let target_count = unsafe {
        (&*(&raw const ROUTES)).targets(message.message_type, message.receiver, &mut targets)
    };
    if !capability_allowed(&message) {
        for receiver in &targets[..target_count] {
            audit(&message, *receiver, 0);
        }
        crate::serial_println!(
            "[IPC] capability denied: sender {} lacks capability {}",
            message.sender,
            message.capability.id
        );
        return Err(ErrorCode::PermissionDenied);
    }
    if target_count == 0 {
        audit(&message, message.receiver, 0);
        crate::serial_println!("[IPC] route denied: type {}", message.message_type);
        return Err(ErrorCode::NotFound);
    }
    for receiver in &targets[..target_count] {
        if !allowed(&message, *receiver) {
            audit(&message, *receiver, 0);
            crate::serial_println!(
                "[IPC] permission denied: {} -> {} (capability {})",
                message.sender,
                receiver,
                message.capability.id
            );
            return Err(ErrorCode::PermissionDenied);
        }
    }

    // All-or-nothing admission: every target queue must have room before
    // the first message is enqueued.
    unsafe {
        let queues = queues();
        for receiver in &targets[..target_count] {
            if queues
                .get(receiver)
                .is_some_and(|queue| queue.len() >= QUEUE_CAPACITY)
            {
                audit(&message, *receiver, 0);
                return Err(ErrorCode::QueueFull);
            }
        }
        for receiver in &targets[..target_count] {
            let mut routed = message;
            routed.receiver = *receiver;
            queues.entry(*receiver).or_default().push_back(routed);
            audit(&message, *receiver, 1);
        }
    }
    for receiver in &targets[..target_count] {
        crate::process::wake_receiver(*receiver);
    }
    Ok(())
}

pub fn recv(receiver: u16) -> Result<Message, ErrorCode> {
    unsafe {
        queues()
            .get_mut(&receiver)
            .and_then(|queue| queue.pop_front())
            .ok_or(ErrorCode::QueueEmpty)
    }
}

pub fn init() {
    load_permissions(&DEFAULT_PERMISSIONS);
    load_routes(&DEFAULT_ROUTES);
    crate::serial_println!("[IPC] audit: configurable permission checks enabled");
}

pub fn test_message() -> Message {
    let mut payload = [0; PAYLOAD_SIZE];
    payload[..12].copy_from_slice(b"hello from A");
    Message {
        sender: 1,
        receiver: 2,
        message_type: message_type::REQUEST,
        capability: Capability {
            id: capability::IPC_SEND_CAPABILITY,
        },
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BROADCAST_RECEIVER, MAX_PERMISSION_RULES, PermissionConfig, PermissionRule, RouteConfig,
        RouteRule, message_type,
    };
    use crate::capability::Capability;

    #[test]
    fn permission_table_changes_runtime_decision() {
        let config = PermissionConfig::with_rules(&[PermissionRule {
            sender: 7,
            receiver: 9,
            capability: Capability { id: 3 },
        }]);
        assert_eq!(config.count, 1);
        assert_eq!(
            config.rules[0],
            PermissionRule {
                sender: 7,
                receiver: 9,
                capability: Capability { id: 3 }
            }
        );
        assert!(config.allows(7, 9, Capability { id: 3 }));
        assert!(!config.allows(7, 9, Capability { id: 4 }));
        assert!(!config.allows(1, 2, Capability { id: 3 }));
        assert!(config.count as usize <= MAX_PERMISSION_RULES);
    }

    #[test]
    fn permission_config_is_bounded() {
        let rules = [PermissionRule {
            sender: 1,
            receiver: 2,
            capability: Capability { id: 1 },
        }; MAX_PERMISSION_RULES + 1];
        let config = PermissionConfig::with_rules(&rules);
        assert_eq!(config.count as usize, MAX_PERMISSION_RULES);
    }

    #[test]
    fn message_type_routes_and_broadcasts() {
        let config = RouteConfig::with_rules(&[
            RouteRule {
                message_type: message_type::BROADCAST,
                receiver: 2,
            },
            RouteRule {
                message_type: message_type::BROADCAST,
                receiver: 3,
            },
        ]);
        let mut targets = [0; super::MAX_ROUTE_RULES];
        assert_eq!(
            config.targets(message_type::BROADCAST, BROADCAST_RECEIVER, &mut targets),
            2
        );
        assert_eq!(&targets[..2], &[2, 3]);
    }
}
