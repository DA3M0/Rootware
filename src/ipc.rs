//! Beta 2 IPC: fixed-size queues with configurable permission rules.

pub const PAYLOAD_SIZE: usize = 32;
pub const MSG_CAPACITY: usize = 8;
pub const MAX_PERMISSION_RULES: usize = 8;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Message {
    pub sender: u16,
    pub receiver: u16,
    pub message_type: u16,
    pub payload: [u8; PAYLOAD_SIZE],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PermissionRule {
    pub sender: u16,
    pub receiver: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PermissionConfig {
    pub rules: [PermissionRule; MAX_PERMISSION_RULES],
    pub count: u16,
}

impl PermissionConfig {
    pub const fn empty() -> Self {
        Self {
            rules: [PermissionRule {
                sender: 0,
                receiver: 0,
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

    pub fn allows(&self, sender: u16, receiver: u16) -> bool {
        let count = core::cmp::min(self.count as usize, MAX_PERMISSION_RULES);
        self.rules[..count]
            .iter()
            .any(|rule| rule.sender == sender && rule.receiver == receiver)
    }
}

const DEFAULT_PERMISSIONS: PermissionConfig = PermissionConfig::with_rules(&[PermissionRule {
    sender: 1,
    receiver: 2,
}]);

static mut QUEUE: [Option<Message>; MSG_CAPACITY] = [None; MSG_CAPACITY];
static mut HEAD: usize = 0;
static mut TAIL: usize = 0;
static mut COUNT: usize = 0;
static mut PERMISSIONS: PermissionConfig = DEFAULT_PERMISSIONS;

fn allowed(sender: u16, receiver: u16) -> bool {
    unsafe {
        let config = &raw const PERMISSIONS;
        (&*config).allows(sender, receiver)
    }
}

pub fn load_permissions(config: &PermissionConfig) {
    unsafe {
        (&raw mut PERMISSIONS).write(*config);
    }
    crate::serial_println!("[IPC] permission table loaded");
}

pub fn send(message: Message) -> Result<(), ()> {
    if !allowed(message.sender, message.receiver) {
        crate::serial_println!(
            "[IPC] permission denied: {} -> {}",
            message.sender,
            message.receiver
        );
        return Err(());
    }
    unsafe {
        if COUNT == MSG_CAPACITY {
            return Err(());
        }
        let tail = TAIL;
        (*(&raw mut QUEUE))[tail] = Some(message);
        TAIL = (tail + 1) % MSG_CAPACITY;
        COUNT += 1;
    }
    crate::serial_println!(
        "[IPC] message sent: {} -> {}",
        message.sender,
        message.receiver
    );
    Ok(())
}

pub fn recv(receiver: u16) -> Option<Message> {
    unsafe {
        if COUNT == 0 || (*(&raw const QUEUE))[HEAD].is_none() {
            return None;
        }
        let message = (*(&raw mut QUEUE))[HEAD].take();
        if message.is_some_and(|msg| msg.receiver != receiver) {
            return None;
        }
        HEAD = (HEAD + 1) % MSG_CAPACITY;
        COUNT -= 1;
        message
    }
}

pub fn init() {
    load_permissions(&DEFAULT_PERMISSIONS);
    crate::serial_println!("[IPC] audit: configurable permission checks enabled");
}

pub fn test_message() -> Message {
    let mut payload = [0; PAYLOAD_SIZE];
    payload[..12].copy_from_slice(b"hello from A");
    Message {
        sender: 1,
        receiver: 2,
        message_type: 1,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_PERMISSION_RULES, PermissionConfig, PermissionRule};

    #[test]
    fn permission_table_changes_runtime_decision() {
        let config = PermissionConfig::with_rules(&[PermissionRule {
            sender: 7,
            receiver: 9,
        }]);
        assert_eq!(config.count, 1);
        assert_eq!(
            config.rules[0],
            PermissionRule {
                sender: 7,
                receiver: 9
            }
        );
        assert!(config.allows(7, 9));
        assert!(!config.allows(1, 2));
        assert!(config.count as usize <= MAX_PERMISSION_RULES);
    }

    #[test]
    fn permission_config_is_bounded() {
        let rules = [PermissionRule {
            sender: 1,
            receiver: 2,
        }; MAX_PERMISSION_RULES + 1];
        let config = PermissionConfig::with_rules(&rules);
        assert_eq!(config.count as usize, MAX_PERMISSION_RULES);
    }
}
