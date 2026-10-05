use core::fmt;

pub use rootware_abi::ErrorCode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error(pub ErrorCode);

pub type Result<T> = core::result::Result<T, Error>;

impl Error {
    pub const fn new(code: ErrorCode) -> Self { Self(code) }

    /// The kernel error code carried by this error.
    pub const fn code(&self) -> ErrorCode { self.0 }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rootware error {:?}", self.0)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
