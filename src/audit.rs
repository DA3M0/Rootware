//! Fixed-size persistent-in-memory IPC audit log.

pub const AUDIT_CAPACITY: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditEntry {
    pub timestamp: u64,
    pub sender: u16,
    pub receiver: u16,
    pub message_type: u16,
    pub result: u16,
    pub capability: u32,
}

static mut LOG: [Option<AuditEntry>; AUDIT_CAPACITY] = [None; AUDIT_CAPACITY];
static mut NEXT: usize = 0;
static mut COUNT: usize = 0;

pub fn record(entry: AuditEntry) {
    unsafe {
        let log = &raw mut LOG;
        (*log)[NEXT] = Some(entry);
        NEXT = (NEXT + 1) % AUDIT_CAPACITY;
        if COUNT < AUDIT_CAPACITY {
            COUNT += 1;
        }
    }
}

pub fn query_sender(sender: u16, output: &mut [AuditEntry; AUDIT_CAPACITY]) -> usize {
    unsafe {
        let log = &raw const LOG;
        let count = COUNT;
        let start = (NEXT + AUDIT_CAPACITY - count) % AUDIT_CAPACITY;
        let mut found = 0;
        for offset in 0..count {
            let entry = (*log)[(start + offset) % AUDIT_CAPACITY];
            if entry.is_some_and(|entry| entry.sender == sender) {
                output[found] = entry.unwrap();
                found += 1;
            }
        }
        found
    }
}

pub fn init() {
    unsafe {
        (&raw mut LOG).write([None; AUDIT_CAPACITY]);
        NEXT = 0;
        COUNT = 0;
    }
    crate::serial_println!("[AUDIT] IPC audit log initialized");
}

#[cfg(test)]
mod tests {
    use super::{query_sender, record, AuditEntry, AUDIT_CAPACITY};

    #[test]
    fn records_and_queries_by_sender() {
        let mut output = [AuditEntry {
            timestamp: 0,
            sender: 0,
            receiver: 0,
            message_type: 0,
            result: 0,
            capability: 0,
        }; AUDIT_CAPACITY];
        record(AuditEntry {
            timestamp: 42,
            sender: 7,
            receiver: 9,
            message_type: 1,
            result: 1,
            capability: 3,
        });
        assert_eq!(query_sender(7, &mut output), 1);
        assert_eq!(output[0].timestamp, 42);
    }
}
