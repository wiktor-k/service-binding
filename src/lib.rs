#![doc = include_str!("../README.md")]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

mod service;

use std::net::AddrParseError;
use std::num::ParseIntError;

pub use service::Binding;
pub use service::Listener;
pub use service::Stream;

/// Errors while processing service listeners.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// IP address cannot be parsed.
    BadAddress(AddrParseError),

    /// Descriptor value cannot be parsed to a number.
    BadDescriptor(ParseIntError),

    /// Descriptor value exceeds acceptable range.
    DescriptorOutOfRange(i32),

    /// Descriptor environment variable (`LISTEN_FDS`) is missing.
    DescriptorsMissing,

    /// Specified URI scheme is not supported.
    UnsupportedScheme,
}

impl From<AddrParseError> for Error {
    fn from(error: AddrParseError) -> Self {
        Error::BadAddress(error)
    }
}

impl From<ParseIntError> for Error {
    fn from(error: ParseIntError) -> Self {
        Error::BadDescriptor(error)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        format!("{}", Error::UnsupportedScheme);
    }
}
