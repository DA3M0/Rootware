//! Frozen syscall number table.
//!
//! Numbers 1–3 are unchanged since the Beta ABI. Numbers 4–8 were
//! added for the 1.0 release and are frozen from now on. Numbers 9–10
//! add the RKM driver framework; future growth appends new numbers and
//! never reuses or reorders existing ones.

pub const SYS_IPC_SEND: u64 = 1;
pub const SYS_IPC_RECEIVE: u64 = 2;
pub const SYS_IPC_REPLY: u64 = 3;
pub const SYS_VERSION: u64 = 4;
pub const SYS_CONSOLE_WRITE: u64 = 5;
pub const SYS_EXIT: u64 = 6;
pub const SYS_SPAWN: u64 = 7;
pub const SYS_CAP_REQUEST: u64 = 8;
pub const SYS_MODULE_REGISTER: u64 = 9;
pub const SYS_MODULE_LIST: u64 = 10;

// Calling conventions per syscall (frozen alongside the numbers):
//
// | Number | Name            | rdi                         | rsi                                  | Return in rax                    |
// |--------|-----------------|-----------------------------|--------------------------------------|----------------------------------|
// | 1      | IPC_SEND        | `*const Message`            | —                                    | 0 or `ErrorCode::status()`       |
// | 2      | IPC_RECEIVE     | receiver id (`u16`)         | `*mut Message` out buffer            | 0, `QueueEmpty`, `InvalidArgument` |
// | 3      | IPC_REPLY       | `*const Message` request    | `*const [u8; PAYLOAD_SIZE]` payload  | 0 or `ErrorCode::status()`       |
// | 4      | VERSION         | —                           | —                                    | `ABI_VERSION` (positive)         |
// | 5      | CONSOLE_WRITE   | `*const u8` buffer          | length in bytes                      | 0 or `InvalidArgument`           |
// | 6      | EXIT            | exit code (`i64`)           | —                                    | does not return                  |
// | 7      | SPAWN           | `*const u8` program name    | name length in bytes                 | new process id or error          |
// | 8      | CAP_REQUEST     | capability kind (`u32`)     | —                                    | 0 or `PermissionDenied`          |
// | 9      | MODULE_REGISTER | `*mut RkmModule` (in/out)   | —                                    | 0 or `ErrorCode::status()`       |
// | 10     | MODULE_LIST     | `*mut RkmModule` out array  | array capacity (entries)             | registered count or error        |
