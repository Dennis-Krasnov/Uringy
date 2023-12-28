//! ...
//!
//! Optimized for reads since routes are typically constructed once at startup.

use crate::ecosystem::http::payload::{Method, Request, StatusCode};
use crate::ecosystem::http::{Handler, IntoHandler, Responder};

/// Handle for composing endpoint handlers.
pub struct Router {
    matcher: matchit::Router<MethodRouter>,
    fallback: Handler,
}

impl Router {
    /// ...
    pub fn new() -> Self {
        Router {
            matcher: matchit::Router::new(),
            fallback: (|r: Responder| r.send(StatusCode::NotFound)).into_handler(),
        }
    }

    /// Adds a route to the router.
    pub fn route(mut self, path: &str, method_router: MethodRouter) -> Self {
        match self.matcher.at_mut(path) {
            Ok(found) => found.value.merge_inner(method_router),
            Err(_) => self.matcher.insert(path, method_router).unwrap(),
        }

        self
    }

    /// Override the the default fallback service that's called if no routes match the request.
    pub fn fallback<ARGS>(mut self, handler: impl IntoHandler<ARGS> + 'static) -> Self {
        // TODO: decide what to do when merging/nesting two routers.
        self.fallback = handler.into_handler();
        self
    }

    /// ...
    pub fn merge(self, other: Self) -> Self {
        unimplemented!();
        self
    }

    // TODO: middleware needs to iterate over existing values https://github.com/ibraheemdev/matchit/issues/9

    /// ...
    pub(crate) fn handle(&self, r: Responder, request: &Request) {
        let handler = self
            .matcher
            .at(request.path)
            .ok()
            .and_then(|found| found.value.route(request.method))
            .unwrap_or(&self.fallback);

        handler(r, request);
    }
}

/// ...
pub struct MethodRouter {
    // HTTP methods
    get: Option<Handler>,
    post: Option<Handler>,
    head: Option<Handler>,
    put: Option<Handler>,
    delete: Option<Handler>,
    connect: Option<Handler>,
    options: Option<Handler>,
    trace: Option<Handler>,
    patch: Option<Handler>,
    // Miscellaneous
    head_derived_from_get: Option<Handler>,
    allowed_methods: String,
    other_method_allowed: Option<Handler>,
}

impl MethodRouter {
    fn new() -> Self {
        MethodRouter {
            get: None,
            post: None,
            head: None,
            put: None,
            delete: None,
            connect: None,
            options: None,
            trace: None,
            patch: None,
            head_derived_from_get: None,
            allowed_methods: String::new(),
            other_method_allowed: None,
        }
    }

    fn set_get(&mut self, handler: Handler) {
        assert!(self.get.is_none());
        self.get = Some(handler);

        if self.head.is_none() {
            // TODO: clone handler (need Rc or + Clone) strip response body from middleware
            // self.head_derived_from_get = Some((|r: Responder| r.send(())).into_handler());
        }

        self.append_allowed_method("GET, HEAD");
    }

    fn set_post(&mut self, handler: Handler) {
        assert!(self.post.is_none());
        self.post = Some(handler);
        self.append_allowed_method("POST");
    }

    fn set_head(&mut self, handler: Handler) {
        assert!(self.head.is_none());
        self.head = Some(handler);

        self.head_derived_from_get = None;

        self.append_allowed_method("HEAD");
    }

    fn set_put(&mut self, handler: Handler) {
        assert!(self.put.is_none());
        self.put = Some(handler);
        self.append_allowed_method("PUT");
    }

    fn set_delete(&mut self, handler: Handler) {
        assert!(self.delete.is_none());
        self.delete = Some(handler);
        self.append_allowed_method("DELETE");
    }

    fn set_connect(&mut self, handler: Handler) {
        assert!(self.connect.is_none());
        self.connect = Some(handler);
        self.append_allowed_method("CONNECT");
    }

    fn set_options(&mut self, handler: Handler) {
        assert!(self.options.is_none());
        self.options = Some(handler);
        self.append_allowed_method("OPTIONS");
    }

    fn set_trace(&mut self, handler: Handler) {
        assert!(self.trace.is_none());
        self.trace = Some(handler);
        self.append_allowed_method("TRACE");
    }

    fn set_patch(&mut self, handler: Handler) {
        assert!(self.patch.is_none());
        self.patch = Some(handler);
        self.append_allowed_method("PATCH");
    }

    fn merge_inner(&mut self, other: Self) {
        if let Some(handler) = other.get {
            self.set_get(handler);
        }
        if let Some(handler) = other.post {
            self.set_post(handler);
        }
        if let Some(handler) = other.head {
            self.set_head(handler);
        }
        if let Some(handler) = other.put {
            self.set_put(handler);
        }
        if let Some(handler) = other.delete {
            self.set_delete(handler);
        }
        if let Some(handler) = other.connect {
            self.set_connect(handler);
        }
        if let Some(handler) = other.options {
            self.set_options(handler);
        }
        if let Some(handler) = other.trace {
            self.set_trace(handler);
        }
        if let Some(handler) = other.patch {
            self.set_patch(handler);
        }
    }

    fn append_allowed_method(&mut self, method: &str) {
        if !self.allowed_methods.is_empty() {
            self.allowed_methods.push_str(", ");
        }
        self.allowed_methods.push_str(method);
        self.allowed_methods.shrink_to_fit();

        // TODO: include Allow header
        self.other_method_allowed =
            Some((|r: Responder| r.send(StatusCode::MethodNotAllowed)).into_handler());
    }

    fn route(&self, method: Method) -> Option<&Handler> {
        match method {
            Method::Get => self.get.as_ref(),
            Method::Post => self.post.as_ref(),
            Method::Head => self.head.as_ref().or(self.head_derived_from_get.as_ref()),
            Method::Put => self.put.as_ref(),
            Method::Delete => self.delete.as_ref(),
            Method::Connect => self.connect.as_ref(),
            Method::Options => self.options.as_ref(),
            Method::Trace => self.trace.as_ref(),
            Method::Patch => self.patch.as_ref(),
        }
        .or(self.other_method_allowed.as_ref())
    }
}

/// Route GET requests to the given handler.
///
/// Requests with the GET method:
/// - Retrieve data at the target resource.
/// - Shouldn't mutate.
/// - Shouldn't have a body.
#[inline]
pub fn get<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_get(handler.into_handler());
    method_router
}

/// Route POST requests to the given handler.
///
/// Requests with the POST method:
/// - Submit data to the target resource.
/// - Aren't idempotent.
#[inline]
pub fn post<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_post(handler.into_handler());
    method_router
}

/// Route HEAD requests to the given handler.
///
/// Requests with the HEAD method:
/// - Are identical to GET requests, but without the response body.
#[inline]
pub fn head<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_head(handler.into_handler());
    method_router
}

/// Route PUT requests to the given handler.
///
/// Requests with the PUT method:
/// - Replace the target resource.
#[inline]
pub fn put<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_put(handler.into_handler());
    method_router
}

/// Route DELETE requests to the given handler.
///
/// Requests with the DELETE method:
/// - Delete the target resource.
/// - Shouldn't have a body.
#[inline]
pub fn delete<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_delete(handler.into_handler());
    method_router
}

/// Route CONNECT requests to the given handler.
///
/// Requests with the CONNECT method:
/// - Establish a tunnel to the server identified by the target resource.
/// - Shouldn't have a body.
#[inline]
pub fn connect<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_connect(handler.into_handler());
    method_router
}

/// Route OPTIONS requests to the given handler.
///
/// Requests with OPTIONS method:
/// - Describe the endpoints the server supports.
/// - Shouldn't have a body.
#[inline]
pub fn options<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_options(handler.into_handler());
    method_router
}

/// Route TRACE requests to the given handler.
///
/// Requests with the TRACE method:
/// - Perform a message loop-back test along the path to the target resource.
/// - Must not have a body.
#[inline]
pub fn trace<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_trace(handler.into_handler());
    method_router
}

/// Route PATCH requests to the given handler.
///
/// Requests with the PATCH method:
/// - Partially update a resource.
/// - Aren't idempotent.
#[inline]
pub fn patch<ARGS>(handler: impl IntoHandler<ARGS> + 'static) -> MethodRouter {
    let mut method_router = MethodRouter::new();
    method_router.set_patch(handler.into_handler());
    method_router
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecosystem::http::payload::StatusCode;
    use crate::ecosystem::http::server::fake_client::FakeClient;
    use crate::ecosystem::http::Responder;
    use crate::runtime::start;

    #[test]
    fn smoke() {
        start(|| {
            let app = Router::new().route("/", get(|r: Responder| r.send("hello")));
            let mut client = FakeClient::from(app);

            let response = client.get("/", ());
            assert_eq!(response.status, StatusCode::Ok);
            assert_eq!(response.body, b"hello");
        })
        .unwrap();
    }

    #[test]
    fn handles_static_route() {
        start(|| {
            let app = Router::new().route("/", get(|r: Responder| r.send(())));
            let mut client = FakeClient::from(app);

            let response = client.get("/", ());

            assert_eq!(response.status, StatusCode::Ok);
        })
        .unwrap();
    }

    #[test]
    fn prioritizes_static_over_dynamic_route() {
        start(|| {
            let app = Router::new()
                .route("/*dyn", get(|r: Responder| r.send(StatusCode::Forbidden)))
                .route("/", get(|r: Responder| r.send(StatusCode::Accepted)));
            let mut client = FakeClient::from(app);

            let response = client.get("/", ());

            assert_eq!(response.status, StatusCode::Accepted);
        })
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn fails_to_create_duplicate_static_route() {
        start(|| {
            Router::new()
                .route("/", get(|r: Responder| r.send(())))
                .route("/", get(|r: Responder| r.send(())));
        })
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn fails_to_create_duplicate_dynamic_route() {
        start(|| {
            Router::new()
                .route("/*dyn1", get(|r: Responder| r.send(())))
                .route("/*dyn2", get(|r: Responder| r.send(())));
        })
        .unwrap();
    }

    #[test]
    fn returns_404_when_unknown_route() {
        start(|| {
            let app = Router::new();
            let mut client = FakeClient::from(app);

            let response = client.get("/", ());

            assert_eq!(response.status, StatusCode::NotFound);
        })
        .unwrap();
    }

    #[test]
    fn handles_unknown_route_with_fallback_handler() {
        start(|| {
            let app = Router::new().fallback(|r: Responder| r.send(StatusCode::Forbidden));
            let mut client = FakeClient::from(app);

            let response = client.get("/", ());

            assert_eq!(response.status, StatusCode::Forbidden);
        })
        .unwrap();
    }

    #[test]
    fn returns_405_when_wrong_method() {
        start(|| {
            let app = Router::new()
                .route("/", post(|r: Responder| r.send(())))
                .route("/", patch(|r: Responder| r.send(())));
            let mut client = FakeClient::from(app);

            let response = client.get("/", ());

            assert_eq!(response.status, StatusCode::MethodNotAllowed);
            //             assert_eq!(
            //                 String::from_utf8_lossy(response.headers[http::header::ALLOW]),
            //                 "post, patch"
            //             );
        })
        .unwrap();
    }

    #[test]
    fn head_defers_to_get() {
        start(|| {
            let app = Router::new().route("/", get(|r: Responder| r.send("hello")));
            let mut client = FakeClient::from(app);

            let response = client.head("/", ());

            assert_eq!(response.status, StatusCode::Ok);
            assert!(response.body.is_empty());
        })
        .unwrap();
    }

    #[test]
    fn custom_head_overrides_get() {
        start(|| {
            let app = Router::new()
                .route("/", get(|r: Responder| r.send(StatusCode::Forbidden)))
                .route("/", head(|r: Responder| r.send(StatusCode::Accepted)));
            let mut client = FakeClient::from(app);

            let response = client.head("/", ());

            assert_eq!(response.status, StatusCode::Accepted);
        })
        .unwrap();
    }
}
