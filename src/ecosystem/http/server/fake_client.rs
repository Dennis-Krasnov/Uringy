//! ...

use crate::ecosystem::http::payload::{AsBody, Method, Request, Response, StatusCode};
use crate::ecosystem::http::server::route::Router;
use crate::ecosystem::http::{Respond, Responder};
use crate::sync::channel;

/// ...
pub struct FakeClient<S = ()> {
    router: Router<S>,
    response: Option<OwnedResponse>,
}

impl<S> FakeClient<S> {
    /// ...
    #[inline]
    pub fn new(router: Router<S>, state: S) -> Self {
        FakeClient {
            router: router.with_state(state),
            response: None,
        }
    }

    /// Make a GET request.
    #[inline]
    pub fn get<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Get, path)
    }

    /// Make a POST request.
    #[inline]
    pub fn post<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Post, path)
    }

    /// Make a HEAD request.
    #[inline]
    pub fn head<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Head, path)
    }

    /// Make a PUT request.
    #[inline]
    pub fn put<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Put, path)
    }

    /// Make a DELETE request.
    #[inline]
    pub fn delete<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Delete, path)
    }

    /// Make a CONNECT request.
    #[inline]
    pub fn connect<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Connect, path)
    }

    /// Make a OPTIONS request.
    #[inline]
    pub fn options<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Options, path)
    }

    /// Make a TRACE request.
    #[inline]
    pub fn trace<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Trace, path)
    }

    /// Make a PATCH request.
    #[inline]
    pub fn patch<'a>(&'a mut self, path: &'a str) -> FakeRequestBuilder<'a, S> {
        self.request(Method::Patch, path)
    }

    /// Make a request with the given method.
    #[inline]
    pub fn request<'a>(&'a mut self, method: Method, path: &'a str) -> FakeRequestBuilder<'a, S> {
        FakeRequestBuilder {
            client: self,
            method,
            path,
            query: Vec::new(),
            headers: Vec::new(),
        }
    }
}

/// Can't `impl<H: IntoHandler<ARGS>, ARGS> From<H> for FakeClient` since ARGS are unconstrained.
impl From<Router<()>> for FakeClient<()> {
    fn from(router: Router) -> Self {
        FakeClient::new(router, ())
    }
}

/// ...
pub struct FakeRequestBuilder<'a, S> {
    client: &'a mut FakeClient<S>,
    method: Method,
    path: &'a str,
    query: Vec<(&'a str, &'a str)>,
    headers: Vec<(&'a str, &'a [u8])>,
}

impl<'a, S> FakeRequestBuilder<'a, S> {
    /// ...
    #[inline]
    pub fn query(mut self, name: &'a str, value: &'a str) -> Self {
        self.query.push((name, value));
        self
    }

    /// ...
    #[inline]
    pub fn header(mut self, name: &'a str, value: &'a [u8]) -> Self {
        self.headers.push((name, value));
        self
    }

    // /// ...
    // pub fn headers(self) -> Self {
    //     self
    // }

    /// ...
    #[inline]
    pub fn send(self, body: impl AsBody) -> Response<'a> {
        let (tx, rx) = channel::unbounded(); // TODO: channel::oneshot
        let r = Responder::new(FakeResponder(tx));
        let query = serde_urlencoded::to_string(self.query).unwrap();
        let request = Request::new(
            self.method,
            self.path,
            &query,
            self.headers,
            body.contents(),
        );
        self.client.router.handle(r, &request);
        self.client.response = Some(rx.recv().expect("must respond..."));
        Response::from(self.client.response.as_ref().unwrap())
    }
}

struct FakeResponder(channel::Sender<OwnedResponse>);

impl Respond for FakeResponder {
    fn respond(self: Box<Self>, response: Response) {
        self.0.send(OwnedResponse::from(response)).unwrap();
    }
}

/// Simplifies transfer of the [Response] back to the [FakeClient].
struct OwnedResponse {
    status_code: StatusCode,
    headers: Vec<(String, Vec<u8>)>,
    body: Box<[u8]>,
}

impl From<Response<'_>> for OwnedResponse {
    fn from(response: Response<'_>) -> Self {
        OwnedResponse {
            status_code: response.status,
            headers: response
                .headers
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_vec()))
                .collect(),
            body: response.body.to_vec().into_boxed_slice(),
        }
    }
}

impl<'a> From<&'a OwnedResponse> for Response<'a> {
    fn from(response: &'a OwnedResponse) -> Self {
        Response {
            status: response.status_code,
            headers: response
                .headers
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_slice()))
                .collect(),
            body: &response.body,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecosystem::http::payload::StatusCode;
    use crate::ecosystem::http::Responder;
    use crate::runtime::start;

    #[test]
    fn smoke() {
        start(|| {
            let routes = Router::new().route(
                Method::Get,
                "/echo",
                |r: Responder, request: &Request, state: &i32| {
                    assert_eq!(request.raw_query(), "foo=bar&beep=boop");
                    assert_eq!(request.query_params()["foo"], "bar");
                    assert_eq!(request.query("foo"), Some("bar"));
                    assert!(!request.raw_headers().is_empty());
                    assert_eq!(request.headers()["foo"], "hello".as_bytes());
                    assert_eq!(request.header("foo"), Some("hello".as_bytes()));
                    assert_eq!(state, &123);
                    r.send(request.body())
                },
            );
            let mut client = FakeClient::new(routes, 123);

            let response = client
                .get("/echo")
                .query("foo", "bar")
                .query("beep", "boop")
                .header("foo", b"hello")
                .send("hello");
            assert_eq!(response.status, StatusCode::Ok);
            assert_eq!(response.body, b"hello");
        })
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn panics_when_no_response_sent() {
        let routes = Router::new().route(Method::Get, "/", |_: Responder| {});
        let mut client = FakeClient::from(routes);

        client.get("/").send(());
    }
}
