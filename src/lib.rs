#![cfg_attr(feature = "fast_thread_local", feature(thread_local))]

#[cfg(feature = "macros")]
pub use uringy_macros::start;

pub mod circular_buffer;
pub mod ecosystem;
pub mod fs;
pub mod net;
pub mod runtime;
pub mod sync;
pub mod time;

/// ...
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error<E> {
    #[error("original...")]
    Original(#[from] E),

    #[error("cancelled...")]
    Cancelled,
}

impl<E> Error<E> {
    /// ...
    #[inline]
    pub fn map<F: FnOnce(E) -> U, U>(self, f: F) -> Error<U> {
        match self {
            Error::Original(e) => Error::Original(f(e)),
            Error::Cancelled => Error::Cancelled,
        }
    }

    /// ...
    #[inline]
    pub fn and_then<F: FnOnce(E) -> Error<U>, U>(self, f: F) -> Error<U> {
        match self {
            Error::Original(e) => f(e),
            Error::Cancelled => Error::Cancelled,
        }
    }
}

impl Error<std::io::Error> {
    /// ...
    pub fn from_io_error(error: std::io::Error) -> Self {
        match error.raw_os_error().unwrap() {
            libc::ECANCELED => Error::Cancelled,
            _ => Error::Original(error),
        }
    }
}

impl From<Error<std::io::Error>> for std::io::Error {
    fn from(error: Error<std::io::Error>) -> Self {
        match error {
            Error::Original(e) => e,
            Error::Cancelled => std::io::Error::from_raw_os_error(libc::ECANCELED),
        }
    }
}

/// ...
pub type IoResult<T> = Result<T, Error<std::io::Error>>;
pub type CancellableResult<T> = Result<T, Error<()>>;
