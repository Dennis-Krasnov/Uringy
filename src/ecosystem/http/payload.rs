//! ...

use ahash::HashMapExt;
use std::cell::OnceCell;
use std::str::FromStr;

#[derive(Debug)]
pub struct Request<'a> {
    method: Method,
    path: &'a str,
    query: &'a str,
    query_map: OnceCell<ahash::HashMap<&'a str, &'a str>>,
    headers: Vec<(&'a str, &'a [u8])>,
    header_map: OnceCell<ahash::HashMap<String, &'a [u8]>>,
    body: &'a [u8],
}

impl<'a> Request<'a> {
    pub(crate) fn new(
        method: Method,
        path: &'a str,
        query: &'a str,
        headers: Vec<(&'a str, &'a [u8])>,
        body: &'a [u8],
    ) -> Self {
        Request {
            method,
            path,
            query,
            query_map: OnceCell::new(),
            headers,
            header_map: OnceCell::new(),
            body,
        }
    }

    /// ...
    #[inline]
    pub fn method(&self) -> Method {
        self.method
    }

    /// ...
    #[inline]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// ...
    #[inline]
    pub fn path_param(&self, _name: &str) -> &str {
        todo!()
    }

    /// ...
    #[inline]
    pub fn raw_query(&self) -> &str {
        &self.query
    }

    /// ... lazy
    #[inline]
    pub fn query_params(&self) -> &ahash::HashMap<&str, &str> {
        self.query_map
            .get_or_init(|| serde_urlencoded::from_str(self.query).unwrap())
    }

    /// ...
    #[inline]
    pub fn query(&self, name: &str) -> Option<&str> {
        self.query_params().get(name).map(|v| *v)
    }

    /// ...
    #[inline]
    pub fn raw_headers(&self) -> &Vec<(&str, &[u8])> {
        &self.headers
    }

    /// ... lazy
    /// ignores duplicates (takes the last)
    #[inline]
    pub fn headers(&self) -> &ahash::HashMap<String, &[u8]> {
        self.header_map.get_or_init(|| {
            let mut map = ahash::HashMap::with_capacity(self.headers.len());
            for (name, value) in &self.headers {
                map.insert(name.to_ascii_lowercase(), *value);
            }
            map
        })
    }

    /// ...
    /// ignores duplicates (takes the last)
    #[inline]
    pub fn header(&self, name: &str) -> Option<&[u8]> {
        self.headers().get(&name.to_ascii_lowercase()).map(|v| *v)
    }

    /// ...
    #[inline]
    pub fn body(&self) -> &[u8] {
        self.body
    }
}

/// ...
#[derive(Debug)]
pub struct Response<'a> {
    pub status: StatusCode,
    pub headers: Vec<(&'a str, &'a [u8])>,
    pub body: &'a [u8],
}

impl Response<'_> {
    /// ...
    #[inline]
    pub fn header(&self, name: &str) -> Option<&[u8]> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    }
}

/// ...
#[derive(Debug, Copy, Clone)]
pub enum Method {
    /// Requests with the GET method:
    /// - Retrieve data at the target resource.
    /// - Shouldn't mutate.
    /// - Shouldn't have a body.
    Get,
    /// Requests with the POST method:
    /// - Submit data to the target resource.
    /// - Aren't idempotent.
    Post,
    /// Requests with the HEAD method:
    /// - Are identical to GET requests, but without the response body.
    Head,
    /// Requests with the PUT method:
    /// - Replace the target resource.
    Put,
    /// Requests with the DELETE method:
    /// - Delete the target resource.
    /// - Shouldn't have a body.
    Delete,
    /// Requests with the CONNECT method:
    /// - Establish a tunnel to the server identified by the target resource.
    /// - Shouldn't have a body.
    Connect,
    /// Requests with OPTIONS method:
    /// - Describe the endpoints the server supports.
    /// - Shouldn't have a body.
    Options,
    /// Requests with the TRACE method:
    /// - Perform a message loop-back test along the path to the target resource.
    /// - Must not have a body.
    Trace,
    /// Requests with the PATCH method:
    /// - Partially update a resource.
    /// - Aren't idempotent.
    Patch,
}

impl FromStr for Method {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Method::Get),
            "POST" => Ok(Method::Post),
            "HEAD" => Ok(Method::Head),
            "PUT" => Ok(Method::Put),
            "DELETE" => Ok(Method::Delete),
            "CONNECT" => Ok(Method::Connect),
            "OPTIONS" => Ok(Method::Options),
            "TRACE" => Ok(Method::Trace),
            "PATCH" => Ok(Method::Patch),
            _ => Err(()),
        }
    }
}

/// ...
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum StatusCode {
    // TODO: rename to Status ???
    Ok,
    Accepted,
    NotModified,
    TemporaryRedirect,
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
}

impl From<StatusCode> for u16 {
    fn from(status: StatusCode) -> Self {
        match status {
            StatusCode::Ok => 200,
            StatusCode::Accepted => 202,
            StatusCode::NotModified => 304,
            StatusCode::TemporaryRedirect => 307,
            StatusCode::BadRequest => 400,
            StatusCode::Unauthorized => 401,
            StatusCode::Forbidden => 403,
            StatusCode::NotFound => 404,
            StatusCode::MethodNotAllowed => 405,
        }
    }
}

/// ...
pub trait AsBody {
    /// ...
    fn contents(&self) -> &[u8];

    /// ...
    fn content_type(&self) -> Option<&str>;
}

impl AsBody for () {
    fn contents(&self) -> &[u8] {
        &[]
    }

    fn content_type(&self) -> Option<&str> {
        None
    }
}

impl AsBody for &str {
    fn contents(&self) -> &[u8] {
        self.as_bytes()
    }

    fn content_type(&self) -> Option<&str> {
        Some("text/plain")
    }
}

impl AsBody for String {
    fn contents(&self) -> &[u8] {
        self.as_bytes()
    }

    fn content_type(&self) -> Option<&str> {
        Some("text/plain")
    }
}

impl AsBody for &[u8] {
    fn contents(&self) -> &[u8] {
        self
    }

    fn content_type(&self) -> Option<&str> {
        Some("application/octet-stream")
    }
}

impl<const N: usize> AsBody for &[u8; N] {
    fn contents(&self) -> &[u8] {
        *self
    }

    fn content_type(&self) -> Option<&str> {
        Some("application/octet-stream")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_body_impls() {
        ().contents();
        "".contents();
        "".as_bytes().contents();
    }

    #[test]
    fn case_insensitive_request_headers() {
        let request = Request::new(
            Method::Get,
            "/",
            "",
            vec![("FOO", b"bar"), ("abc", b"xyz")],
            b"",
        );

        assert_eq!(request.header("foo"), Some("bar".as_bytes()));
        assert_eq!(request.header("ABC"), Some("xyz".as_bytes()));
    }

    // TODO: case_insensitive_response_headers
}
