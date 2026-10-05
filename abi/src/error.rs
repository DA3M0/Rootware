//! Stable error codes and their syscall status encoding.

/// Error codes shared by the kernel and user programs.
///
/// A syscall reports failure through the negative of the discriminant
/// in `rax`; success is always `0`.
#[repr(i64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    /// A pointer, length or identifier argument was invalid.
    InvalidArgument = 1,
    /// The caller lacks the capability or permission for the operation.
    PermissionDenied = 2,
    /// A destination queue has no free capacity.
    QueueFull = 3,
    /// A source queue had no message to deliver.
    QueueEmpty = 4,
    /// The requested route, program or object does not exist.
    NotFound = 5,
    /// The syscall number is unknown to the running kernel.
    Unsupported = 6,
    /// The transport layer failed for a reason not covered above.
    Transport = 7,
}

impl ErrorCode {
    /// Status code carried in `rax` after a failed syscall.
    pub const fn status(self) -> i64 {
        -(self as i64)
    }

    /// Inverse of [`status`]. `0` means success and yields `None`.
    pub const fn from_status(status: i64) -> Option<Self> {
        match -status {
            1 => Some(Self::InvalidArgument),
            2 => Some(Self::PermissionDenied),
            3 => Some(Self::QueueFull),
            4 => Some(Self::QueueEmpty),
            5 => Some(Self::NotFound),
            6 => Some(Self::Unsupported),
            7 => Some(Self::Transport),
            _ => None,
        }
    }
}
