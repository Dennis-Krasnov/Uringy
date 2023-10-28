//! ...

use std::fmt::{Debug, Formatter};
use std::io::{Cursor, Read};

use http::header;

use crate::ecosystem::http::into_body::IntoBody;

/// ...
pub struct Response {
    parts: Parts,
    body: Box<dyn Read>,
}

impl Response {
    /// ...
    #[inline]
    pub fn builder() -> Builder {
        Builder::new()
    }

    // TODO: remove this, only use builder!
    /// ...
    #[inline]
    pub fn new(body: Box<dyn Read>) -> Self {
        Response {
            parts: Parts::new(),
            body,
        }
    }

    /// ...
    #[inline]
    pub fn from_parts(parts: Parts, body: Box<dyn Read>) -> Self {
        Response { parts, body }
    }

    /// ...
    #[inline]
    pub fn status(&self) -> &http::StatusCode {
        &self.parts.status
    }

    /// ...
    #[inline]
    pub fn status_mut(&mut self) -> &mut http::StatusCode {
        &mut self.parts.status
    }

    /// ...
    #[inline]
    pub fn version(&self) -> &http::Version {
        &self.parts.version
    }

    /// ...
    #[inline]
    pub fn version_mut(&mut self) -> &mut http::Version {
        &mut self.parts.version
    }

    /// ...
    #[inline]
    pub fn headers(&self) -> &http::HeaderMap {
        &self.parts.headers
    }

    /// ...
    #[inline]
    pub fn headers_mut(&mut self) -> &mut http::HeaderMap {
        &mut self.parts.headers
    }

    /// ...
    #[inline]
    pub fn extensions(&self) -> &http::Extensions {
        &self.parts.extensions
    }

    /// ...
    #[inline]
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        &mut self.parts.extensions
    }

    /// ...
    #[inline]
    pub fn into_body(self) -> Box<dyn Read> {
        self.body
    }

    /// ...
    #[inline]
    pub fn into_parts(self) -> (Parts, Box<dyn Read>) {
        (self.parts, self.body)
    }

    // TODO: optionally respect charset header
    /// ...
    pub fn into_string(mut self) -> crate::IoResult<String> {
        let mut string = String::new();
        self.body
            .read_to_string(&mut string)
            .map_err(crate::Error::from_io_error)?;
        Ok(string)
    }

    /// ...
    pub fn into_vec(mut self) -> crate::IoResult<Vec<u8>> {
        let mut buffer = Vec::new();
        self.body
            .read_to_end(&mut buffer)
            .map_err(crate::Error::from_io_error)?;
        Ok(buffer)
    }
}

impl Debug for Response {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status())
            .field("version", &self.version())
            .field("headers", &self.headers())
            // omits Extensions because they're not useful
            .finish()
    }
}

/// ...
pub struct Parts {
    /// The response's status
    pub status: http::StatusCode,

    /// The response's version
    pub version: http::Version,

    /// The response's headers
    pub headers: http::HeaderMap<http::HeaderValue>,

    /// The response's extensions
    pub extensions: http::Extensions,

    /// Stop user from instantiating [Parts].
    _private: (),
}

impl Parts {
    fn new() -> Self {
        Parts {
            status: http::StatusCode::default(),
            version: http::Version::default(),
            headers: http::HeaderMap::default(),
            extensions: http::Extensions::default(),
            _private: (),
        }
    }
}

/// ...
pub struct Builder(http::Result<Parts>);

impl Builder {
    /// ...
    pub fn new() -> Self {
        Builder(Ok(Parts::new()))
    }

    /// ...
    pub fn status<T>(mut self, status: T) -> Self
    where
        T: TryInto<http::StatusCode>,
        T::Error: Into<http::Error>,
    {
        if let Ok(parts) = &mut self.0 {
            match status.try_into() {
                Ok(status) => parts.status = status,
                Err(error) => self = Builder(Err(error.into())),
            }
        }

        self
    }

    /// ...
    pub fn version(mut self, version: http::Version) -> Self {
        if let Ok(parts) = &mut self.0 {
            parts.version = version;
        }

        self
    }

    /// ...
    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: TryInto<http::HeaderName>,
        K::Error: Into<http::Error>,
        V: TryInto<http::HeaderValue>,
        V::Error: Into<http::Error>,
    {
        if let Ok(parts) = &mut self.0 {
            match (key.try_into(), value.try_into()) {
                (Ok(key), Ok(value)) => {
                    parts.headers.append(key, value);
                }
                (Err(error), _) => self = Builder(Err(error.into())),
                (_, Err(error)) => self = Builder(Err(error.into())),
            }
        }

        self
    }

    // TODO: extension https://docs.rs/http/latest/src/http/request.rs.html#988

    // TODO: ref functions

    /// ...
    pub fn body(mut self, body: impl IntoBody) -> http::Result<Response> {
        let content_type = body.content_type();
        let (length, body) = body.into_body();

        if let Ok(parts) = &mut self.0 {
            if let Some(content_type) = content_type {
                parts.headers.insert(
                    header::CONTENT_TYPE,
                    header::HeaderValue::from_str(content_type.as_ref()).unwrap(),
                );
            }

            if let Some(length) = length {
                parts.headers.insert(
                    header::CONTENT_LENGTH,
                    header::HeaderValue::from_str(&length.to_string()).unwrap(),
                );
            } else {
                todo!("chunked encoding?");
            }
        }

        self.0.map(|parts| Response { parts, body })
    }

    /// Avoid unconditionally setting content type to octet-stream when deserializing.
    pub(crate) fn raw_body(self, body: Vec<u8>) -> http::Result<Response> {
        self.0.map(|parts| Response {
            parts,
            body: Box::new(Cursor::new(body)),
        })
    }
}
