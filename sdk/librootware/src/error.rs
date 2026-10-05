use core::fmt;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidArgument = 1,
    PermissionDenied = 2,
    QueueFull = 3,
    QueueEmpty = 4,
    NotFound = 5,
    Unsupported = 6,
    Transport = 7,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error(pub ErrorCode);

pub type Result<T> = core::result::Result<T, Error>;

impl Error {
    pub const fn new(code: ErrorCode) -> Self { Self(code) }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rootware error {:?}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
