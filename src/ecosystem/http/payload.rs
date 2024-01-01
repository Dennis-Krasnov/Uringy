//! ...

// FIXME: only server should have path variables (fake client directly against fn has none...) (Option?)

use std::str::FromStr;

#[derive(Debug)]
pub struct Request<'a> {
    pub method: Method,
    pub path: &'a str,
    pub query: &'a str,
    pub headers: Vec<(&'a str, &'a [u8])>,
    pub body: &'a [u8],
}

/// ...
pub trait AsRequest {
    /// ...
    fn as_request<'a>(&'a self, method: Method, path: &'a str, query: &'a str) -> Request<'a>;
}

impl<B: AsBody> AsRequest for B {
    fn as_request<'a>(&'a self, method: Method, path: &'a str, query: &'a str) -> Request<'a> {
        Request {
            method,
            path,
            query,
            headers: Vec::new(),
            body: self.contents(),
        }
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
    // /// ...
    // pub fn status(&self) -> StatusCode {
    //     self.status
    // }
    //
    // /// ...
    // pub fn body(&self) -> &[u8] {
    //     self.body
    // }
}

/// Flexible construction of `Response`.
///
/// Accepts:
/// - individual `StatusCode`.
/// - individual `impl IntoResponseParts`.
/// - individual `impl IntoBody`.
/// - tuple of (0..1 `StatusCode`, 0..15 `impl IntoResponseParts`, 0..1 `impl IntoBody`).
pub trait AsResponse {
    /// ...
    fn as_response(&self) -> Response; // TODO: returns tuple of parts
}

impl AsResponse for StatusCode {
    fn as_response(&self) -> Response {
        Response {
            status: self.clone(),
            headers: Vec::new(),
            body: &[],
        }
    }
}

impl<B: AsBody> AsResponse for B {
    fn as_response(&self) -> Response {
        let mut headers = Vec::new();
        // headers.push((
        //     "content-length",
        //     self.contents().len().to_string().as_bytes(),
        // ));
        // TODO: override if already exists
        if let Some(content_type) = self.content_type() {
            headers.push(("content-type", content_type.as_bytes()));
        }

        Response {
            status: StatusCode::Ok,
            headers,
            body: self.contents(),
        }
    }
}

impl<const N: usize> AsResponse for (StatusCode, [(&str, &[u8]); N]) {
    fn as_response(&self) -> Response {
        let (status, headers) = self;

        Response {
            status: status.clone(),
            headers: Vec::from(headers),
            body: &[],
        }
    }
}

impl<const N: usize, B: AsBody> AsResponse for ([(&str, &[u8]); N], B) {
    fn as_response(&self) -> Response {
        let (headers, body) = self;

        let mut headers = Vec::from(headers);

        if let Some(content_type) = body.content_type() {
            headers.push(("content-type", content_type.as_bytes()));
        }

        Response {
            status: StatusCode::Ok,
            headers,
            body: body.contents(),
        }
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
    fn as_request_impls() {
        ().as_request(Method::Get, "/", "");
        "".as_request(Method::Get, "/", "");
        "".as_bytes().as_request(Method::Get, "/", "");
    }

    #[test]
    fn as_response_impls() {
        StatusCode::Ok.as_response();

        ([], "").as_response();

        ().as_response();
        "".as_response();
        "".as_bytes().as_response();
    }

    #[test]
    fn as_body_impls() {
        ().contents();
        "".contents();
        "".as_bytes().contents();
    }
}
