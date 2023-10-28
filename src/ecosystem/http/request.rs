//! ...

use std::fmt::{Debug, Formatter};
use std::io::{Cursor, Read};

use http::header;

use crate::ecosystem::http::into_body::IntoBody;

/// ...
pub struct Request {
    parts: Parts,
    body: Box<dyn Read>, // TODO: type alias
}

impl Request {
    /// ...
    #[inline]
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// ...
    pub fn get<T>(uri: T) -> Builder
    where
        T: TryInto<http::Uri>,
        T::Error: Into<http::Error>,
    {
        Builder::new().method(http::Method::GET).uri(uri)
    }

    /// ...
    pub fn post<T>(uri: T) -> Builder
    where
        T: TryInto<http::Uri>,
        T::Error: Into<http::Error>,
    {
        Builder::new().method(http::Method::POST).uri(uri)
    }

    // TODO: remove this, only use builder
    /// ...
    #[inline]
    pub fn new(body: Box<dyn Read>) -> Self {
        Request {
            parts: Parts::new(),
            body,
        }
    }

    /// ...
    #[inline]
    pub fn from_parts(parts: Parts, body: Box<dyn Read>) -> Self {
        Request { parts, body }
    }

    /// ...
    #[inline]
    pub fn method(&self) -> &http::Method {
        &self.parts.method
    }

    /// ...
    #[inline]
    pub fn method_mut(&mut self) -> &mut http::Method {
        &mut self.parts.method
    }

    /// ...
    #[inline]
    pub fn uri(&self) -> &http::Uri {
        &self.parts.uri
    }

    /// ...
    #[inline]
    pub fn uri_mut(&mut self) -> &mut http::Uri {
        &mut self.parts.uri
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
    } // TODO: rename to into_reader? place after into_vec

    /// ...
    #[inline]
    pub fn into_parts(self) -> (Parts, Box<dyn Read>) {
        (self.parts, self.body)
    } // FIXME: remove??

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

impl Debug for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method())
            .field("uri", &self.uri())
            .field("version", &self.version())
            .field("headers", &self.headers())
            // omits Extensions because they're not useful
            .finish()
    }
}

/// ...
pub struct Parts {
    /// The request's method
    pub method: http::Method,

    /// The request's URI
    pub uri: http::Uri,

    /// The request's version
    pub version: http::Version,

    /// The request's headers
    pub headers: http::HeaderMap<http::HeaderValue>,

    /// The request's extensions
    pub extensions: http::Extensions,

    /// Stop user from instantiating [Parts].
    _private: (),
}

impl Parts {
    fn new() -> Self {
        Parts {
            method: http::Method::default(),
            uri: http::Uri::default(),
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
    pub fn method<T>(mut self, method: T) -> Self
    where
        T: TryInto<http::Method>,
        T::Error: Into<http::Error>,
    {
        if let Ok(parts) = &mut self.0 {
            match method.try_into() {
                Ok(method) => parts.method = method,
                Err(error) => self = Builder(Err(error.into())),
            }
        }

        self
    }

    /// ...
    pub fn uri<T>(mut self, uri: T) -> Self
    where
        T: TryInto<http::Uri>,
        T::Error: Into<http::Error>,
    {
        if let Ok(parts) = &mut self.0 {
            match uri.try_into() {
                Ok(uri) => parts.uri = uri,
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
    pub fn body(mut self, body: impl IntoBody) -> http::Result<Request> {
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

        self.0.map(|parts| Request { parts, body })
    }

    /// Avoid unconditionally setting content type to octet-stream when deserializing.
    pub(crate) fn raw_body(self, body: Vec<u8>) -> http::Result<Request> {
        self.0.map(|parts| Request {
            parts,
            body: Box::new(Cursor::new(body)),
        })
    }
}
