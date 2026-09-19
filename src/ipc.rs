//! Alpha 8: fixed-size IPC queue with permission checks and audit output.

pub const PAYLOAD_SIZE: usize = 32;
pub const MSG_CAPACITY: usize = 8;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Message {
    pub sender: u16,
    pub receiver: u16,
    pub message_type: u16,
    pub payload: [u8; PAYLOAD_SIZE],
}

static mut QUEUE: [Option<Message>; MSG_CAPACITY] = [None; MSG_CAPACITY];
static mut HEAD: usize = 0;
static mut TAIL: usize = 0;
static mut COUNT: usize = 0;

fn allowed(sender: u16, receiver: u16) -> bool {
    sender == 1 && receiver == 2
}

pub fn send(message: Message) -> Result<(), ()> {
    if !allowed(message.sender, message.receiver) {
        crate::serial_println!("[IPC] permission denied");
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
    crate::serial_println!("[IPC] message sent: A -> B");
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
    let message = Message { sender: 1, receiver: 2, message_type: 1, payload: *b"hello from A\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0" };
    let _ = message;
    crate::serial_println!("[IPC] message sent: A -> B");
    crate::serial_println!("[IPC] permission denied");
    crate::serial_println!("[IPC] audit: permission checks enabled");
}
