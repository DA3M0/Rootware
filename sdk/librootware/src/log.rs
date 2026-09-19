#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level { Error, Warn, Info, Debug, Trace }

/// Logs a preformatted message without allocating.
pub fn log(level: Level, message: &str) {
    log_args(level, format_args!("{message}"));
}

/// Logs formatting arguments directly.
pub fn log_args(level: Level, args: core::fmt::Arguments<'_>) {
    #[cfg(feature = "std")]
    eprintln!("[ROOTWARE/{level:?}] {args}");
    #[cfg(not(feature = "std"))]
    { let _ = (level, args); }
}

#[macro_export]
macro_rules! info { ($($arg:tt)*) => { $crate::log::log_args($crate::log::Level::Info, format_args!($($arg)*)) } }
#[macro_export]
macro_rules! warn { ($($arg:tt)*) => { $crate::log::log_args($crate::log::Level::Warn, format_args!($($arg)*)) } }
