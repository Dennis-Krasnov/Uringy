#[cfg(feature = "std")]
extern crate std;

use crate::cursor;
use core::fmt::{Display, Formatter};

/// NatsProtoError enumerates all possible errors returned by this library.
#[derive(Debug, PartialEq)]
pub enum NatsProtoError {
    /// ...
    BufferTooSmall,

    /// ...
    InvalidProtocol,
}

impl Display for NatsProtoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match *self {
            NatsProtoError::BufferTooSmall => write!(f, "BufferTooSmall"),
            NatsProtoError::InvalidProtocol => write!(f, "InvalidProtocol"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for NatsProtoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None // TODO
             // match *self {
             //     // WordCountError::EmptySource => None,
             //     // WordCountError::ReadError { ref source } => Some(source),
    }
}

impl From<cursor::BufferTooSmall> for NatsProtoError {
    fn from(_: cursor::BufferTooSmall) -> Self {
        NatsProtoError::BufferTooSmall
    }
}
