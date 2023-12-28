//! ...

use crate::ecosystem::http::payload::{IntoRequest, Method, Response, StatusCode};
use crate::ecosystem::http::server::route::Router;
use crate::ecosystem::http::{Respond, Responder};
use crate::sync::channel;

/// ...
pub struct FakeClient {
    router: Router,
    response: Option<OwnedResponse>,
}

impl FakeClient {
    /// Make a GET request.
    pub fn get(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Get, path, request)
    }

    /// Make a POST request.
    pub fn post(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Post, path, request)
    }

    /// Make a HEAD request.
    pub fn head(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Head, path, request)
    }

    /// Make a PUT request.
    pub fn put(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Put, path, request)
    }

    /// Make a DELETE request.
    pub fn delete(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Delete, path, request)
    }

    /// Make a CONNECT request.
    pub fn connect(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Connect, path, request)
    }

    /// Make a OPTIONS request.
    pub fn options(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Options, path, request)
    }

    /// Make a TRACE request.
    pub fn trace(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Trace, path, request)
    }

    /// Make a PATCH request.
    pub fn patch(&mut self, path: &str, request: impl IntoRequest) -> Response {
        self.request(Method::Patch, path, request)
    }

    /// Make a request with the given method.
    pub fn request(&mut self, method: Method, path: &str, request: impl IntoRequest) -> Response {
        let (tx, rx) = channel::unbounded(); // TODO: channel::oneshot
        let r = Responder(Box::new(FakeResponder(tx)));
        let request = request.into_request(method, path, ""); // FIXME: take query params from builder
        self.router.handle(r, &request);
        self.response = Some(rx.recv().expect("must respond..."));
        Response::from(self.response.as_ref().unwrap())
    }
}

/// Can't `impl<H: IntoHandler<ARGS>, ARGS> From<H> for FakeClient` since ARGS are unconstrained.
impl From<Router> for FakeClient {
    fn from(router: Router) -> Self {
        FakeClient {
            router,
            response: None,
        }
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
    use crate::ecosystem::http::server::route::get;
    use crate::ecosystem::http::Responder;

    #[test]
    fn smoke() {
        let app = Router::new().route("/", get(|r: Responder| r.send(())));
        let mut client = FakeClient::from(app);

        let response = client.get("/", ());

        assert_eq!(response.status, StatusCode::Ok);
    }

    #[test]
    #[should_panic]
    fn panics_when_no_response_sent() {
        let app = Router::new().route("/", get(|_| {}));
        let mut client = FakeClient::from(app);

        client.get("/", ());
    }
}
