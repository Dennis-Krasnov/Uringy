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
pub trait IntoRequest {
    /// ...
    fn into_request<'a>(&'a self, method: Method, path: &'a str, query: &'a str) -> Request;
}

impl<B: IntoBody> IntoRequest for B {
    fn into_request<'a>(&'a self, method: Method, path: &'a str, query: &'a str) -> Request {
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
pub trait IntoResponse {
    /// ...
    fn into_response(&self) -> Response; // TODO: returns tuple of parts
}

impl IntoResponse for StatusCode {
    fn into_response(&self) -> Response {
        Response {
            status: self.clone(),
            headers: Vec::new(),
            body: &[],
        }
    }
}

impl<B: IntoBody> IntoResponse for B {
    fn into_response(&self) -> Response {
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

impl<const N: usize> IntoResponse for (StatusCode, [(&str, &[u8]); N]) {
    fn into_response(&self) -> Response {
        let (status, headers) = self;

        Response {
            status: status.clone(),
            headers: Vec::from(headers),
            body: &[],
        }
    }
}

impl<const N: usize, B: IntoBody> IntoResponse for ([(&str, &[u8]); N], B) {
    fn into_response(&self) -> Response {
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
    Get,
    Post,
    Head,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
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
            StatusCode::Forbidden => 403,
            StatusCode::NotFound => 404,
            StatusCode::MethodNotAllowed => 405,
        }
    }
}

/// ...
pub trait IntoBody {
    /// ...
    fn contents(&self) -> &[u8];

    /// ...
    fn content_type(&self) -> Option<&str>;
}

impl IntoBody for () {
    fn contents(&self) -> &[u8] {
        &[]
    }

    fn content_type(&self) -> Option<&str> {
        None
    }
}

impl IntoBody for &str {
    fn contents(&self) -> &[u8] {
        self.as_bytes()
    }

    fn content_type(&self) -> Option<&str> {
        Some("text/plain")
    }
}

impl IntoBody for &[u8] {
    fn contents(&self) -> &[u8] {
        self
    }

    fn content_type(&self) -> Option<&str> {
        Some("application/octet-stream")
    }
}

impl<const N: usize> IntoBody for &[u8; N] {
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
    fn into_request_impls() {
        ().into_request(Method::Get, "/", "");
        "".into_request(Method::Get, "/", "");
        "".as_bytes().into_request(Method::Get, "/", "");
    }

    #[test]
    fn into_response_impls() {
        StatusCode::Ok.into_response();

        ([], "").into_response();

        ().into_response();
        "".into_response();
        "".as_bytes().into_response();
    }

    #[test]
    fn into_body_impls() {
        ().contents();
        "".contents();
        "".as_bytes().contents();
    }
}
