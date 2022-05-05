#![doc = include_str!("../README.md")]

mod service;
mod endpoint;

pub use service::Binding;
pub use service::Listener;
pub use endpoint::Endpoint;

#[derive(Debug)]
pub struct Error;

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Error
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(_: std::net::AddrParseError) -> Self {
        Error
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}
