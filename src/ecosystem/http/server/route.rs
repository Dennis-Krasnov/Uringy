//! ...
//!
//! Optimized for reads since routes are typically constructed once at startup.

use crate::ecosystem::http::payload::{Method, Request, StatusCode};
use crate::ecosystem::http::{Handler, IntoHandler, Responder};

/// Handle for composing endpoint handlers.
// TODO: #[must_use]
pub struct Router<S = ()> {
    matcher: matchit::Router<MethodRouter<S>>,
    /// Work around for `matchit::Router` not exposing an iterator
    matcher_paths: Vec<String>,
    fallback: Handler<S>,
    state: Option<S>,
}

// TODO: S: Clone + 'static
impl<S> Router<S> {
    /// ...
    #[inline]
    pub fn new() -> Self {
        Router {
            matcher: matchit::Router::new(),
            matcher_paths: vec![],
            fallback: (|r: Responder| r.status(StatusCode::NotFound).send(())).into_handler(),
            state: None,
        }
    }

    pub(crate) fn with_state(mut self, state: S) -> Self {
        assert!(self.state.is_none());
        self.state = Some(state);
        self
    }

    /// Adds a route to the router.
    #[inline]
    pub fn route<ARGS>(
        mut self,
        method: Method,
        path: &str,
        handler: impl IntoHandler<ARGS, S>,
    ) -> Self {
        match self.matcher.at_mut(path) {
            Ok(found) => found.value.set(method, handler.into_handler()),
            Err(_) => {
                let mut method_router = MethodRouter::new();
                method_router.set(method, handler.into_handler());
                self.matcher.insert(path, method_router).unwrap();
                self.matcher_paths.push(path.to_string());
            }
        }

        self
    }

    /// Override the the default fallback service that's called if no routes match the request.
    #[inline]
    pub fn fallback<ARGS>(mut self, handler: impl IntoHandler<ARGS, S> + 'static) -> Self {
        // TODO: decide what to do when merging/nesting two routers.
        self.fallback = handler.into_handler();
        self
    }

    // /// ...
    // #[inline]
    // pub fn merge(self, _other: Self) -> Self {
    //     for path in _other.matcher_paths {
    //
    //     }
    //
    //     unimplemented!();
    // }

    // /// ...
    // #[inline]
    // pub fn nest<S2: Into<S>>(self, other: Router<S2>) -> Self {
    //     // for path in other.matcher_paths {
    //     //     other.matcher.at()
    //     // }
    //
    //     self
    // }

    /// ...
    pub(crate) fn handle(&self, r: Responder, request: &Request) {
        // TODO: take ownership of request
        let handler = self
            .matcher
            .at(request.path())
            .ok()
            .and_then(|found| found.value.handle(request.method())) // TODO: also return params
            .unwrap_or(&self.fallback);

        // TODO: add params to request
        let state = self.state.as_ref().unwrap();

        handler(r, request, state); // TODO: pass reference to request
    }
}

/// ...
pub struct MethodRouter<S> {
    // HTTP methods
    get: Option<Handler<S>>,
    post: Option<Handler<S>>,
    head: Option<Handler<S>>,
    put: Option<Handler<S>>,
    delete: Option<Handler<S>>,
    connect: Option<Handler<S>>,
    options: Option<Handler<S>>,
    trace: Option<Handler<S>>,
    patch: Option<Handler<S>>,
    // Miscellaneous
    head_derived_from_get: Option<Handler<S>>,
    allowed_methods: String,
    other_method_allowed: Option<Handler<S>>,
}

impl<S> MethodRouter<S> {
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

    fn set_get(&mut self, handler: Handler<S>) {
        assert!(self.get.is_none());
        self.get = Some(handler);

        if self.head.is_none() {
            // TODO: clone handler (need Rc or + Clone) strip response body from middleware
            // self.head_derived_from_get = Some((|r: Responder| r.send(())).into_handler());
        }

        self.append_allowed_method("GET, HEAD");
    }

    fn set_post(&mut self, handler: Handler<S>) {
        assert!(self.post.is_none());
        self.post = Some(handler);
        self.append_allowed_method("POST");
    }

    fn set_head(&mut self, handler: Handler<S>) {
        assert!(self.head.is_none());
        self.head = Some(handler);

        self.head_derived_from_get = None;

        self.append_allowed_method("HEAD");
    }

    fn set_put(&mut self, handler: Handler<S>) {
        assert!(self.put.is_none());
        self.put = Some(handler);
        self.append_allowed_method("PUT");
    }

    fn set_delete(&mut self, handler: Handler<S>) {
        assert!(self.delete.is_none());
        self.delete = Some(handler);
        self.append_allowed_method("DELETE");
    }

    fn set_connect(&mut self, handler: Handler<S>) {
        assert!(self.connect.is_none());
        self.connect = Some(handler);
        self.append_allowed_method("CONNECT");
    }

    fn set_options(&mut self, handler: Handler<S>) {
        assert!(self.options.is_none());
        self.options = Some(handler);
        self.append_allowed_method("OPTIONS");
    }

    fn set_trace(&mut self, handler: Handler<S>) {
        assert!(self.trace.is_none());
        self.trace = Some(handler);
        self.append_allowed_method("TRACE");
    }

    fn set_patch(&mut self, handler: Handler<S>) {
        assert!(self.patch.is_none());
        self.patch = Some(handler);
        self.append_allowed_method("PATCH");
    }

    fn append_allowed_method(&mut self, method: &str) {
        if !self.allowed_methods.is_empty() {
            self.allowed_methods.push_str(", ");
        }
        self.allowed_methods.push_str(method);
        self.allowed_methods.shrink_to_fit();

        let allowed_methods = self.allowed_methods.clone();
        self.other_method_allowed = Some(
            (move |r: Responder| {
                r.status(StatusCode::MethodNotAllowed)
                    .header("allow", allowed_methods.as_bytes())
                    .send(())
            })
            .into_handler(),
        );
    }

    fn set(&mut self, method: Method, handler: Handler<S>) {
        match method {
            Method::Get => self.set_get(handler),
            Method::Post => self.set_post(handler),
            Method::Head => self.set_head(handler),
            Method::Put => self.set_put(handler),
            Method::Delete => self.set_delete(handler),
            Method::Connect => self.set_connect(handler),
            Method::Options => self.set_options(handler),
            Method::Trace => self.set_trace(handler),
            Method::Patch => self.set_patch(handler),
        }
    }

    // fn merge(&mut self, other: Self) {
    //     if let Some(handler) = other.get {
    //         self.set_get(handler);
    //     }
    //     if let Some(handler) = other.post {
    //         self.set_post(handler);
    //     }
    //     if let Some(handler) = other.head {
    //         self.set_head(handler);
    //     }
    //     if let Some(handler) = other.put {
    //         self.set_put(handler);
    //     }
    //     if let Some(handler) = other.delete {
    //         self.set_delete(handler);
    //     }
    //     if let Some(handler) = other.connect {
    //         self.set_connect(handler);
    //     }
    //     if let Some(handler) = other.options {
    //         self.set_options(handler);
    //     }
    //     if let Some(handler) = other.trace {
    //         self.set_trace(handler);
    //     }
    //     if let Some(handler) = other.patch {
    //         self.set_patch(handler);
    //     }
    // }

    fn handle(&self, method: Method) -> Option<&Handler<S>> {
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

#[cfg(test)]
mod tests {
    use crate::ecosystem::http::payload::Method;
    use crate::ecosystem::http::payload::StatusCode;
    use crate::ecosystem::http::server::fake_client::FakeClient;
    use crate::ecosystem::http::Responder;
    use crate::runtime::start;

    use super::*;

    #[test]
    fn handles_static_route() {
        start(|| {
            let routes = Router::new().route(Method::Get, "/", |r: Responder| r.send(()));
            let mut client = FakeClient::from(routes);

            let response = client.get("/").send(());

            assert_eq!(response.status, StatusCode::Ok);
        })
        .unwrap();
    }

    #[test]
    fn prioritizes_static_over_dynamic_route() {
        start(|| {
            let routes = Router::new()
                .route(Method::Get, "/*dyn", |r: Responder| {
                    r.status(StatusCode::Forbidden).send(())
                })
                .route(Method::Get, "/", |r: Responder| {
                    r.status(StatusCode::Accepted).send(())
                });
            let mut client = FakeClient::from(routes);

            let response = client.get("/").send(());

            assert_eq!(response.status, StatusCode::Accepted);
        })
        .unwrap();
    }

    #[test]
    fn merges_two_routes_with_same_path_but_different_methods() {
        start(|| {
            let routes = Router::new()
                .route(Method::Post, "/", |r: Responder| r.send(()))
                .route(Method::Put, "/", |r: Responder| r.send(()));
            let mut client = FakeClient::from(routes);

            assert_eq!(client.post("/").send(()).status, StatusCode::Ok);
            assert_eq!(client.put("/").send(()).status, StatusCode::Ok);
        })
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn fails_to_create_duplicate_static_route() {
        start(|| {
            Router::<()>::new()
                .route(Method::Get, "/", |r: Responder| r.send(()))
                .route(Method::Get, "/", |r: Responder| r.send(()));
        })
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn fails_to_create_duplicate_dynamic_route() {
        start(|| {
            Router::<()>::new()
                .route(Method::Get, "/*dyn1", |r: Responder| r.send(()))
                .route(Method::Get, "/*dyn2", |r: Responder| r.send(()));
        })
        .unwrap();
    }

    #[test]
    fn returns_404_when_unknown_route() {
        start(|| {
            let routes = Router::new();
            let mut client = FakeClient::from(routes);

            let response = client.get("/").send(());

            assert_eq!(response.status, StatusCode::NotFound);
        })
        .unwrap();
    }

    #[test]
    fn handles_unknown_route_with_fallback_handler() {
        start(|| {
            let routes =
                Router::new().fallback(|r: Responder| r.status(StatusCode::Forbidden).send(()));
            let mut client = FakeClient::from(routes);

            let response = client.get("/").send(());

            assert_eq!(response.status, StatusCode::Forbidden);
        })
        .unwrap();
    }

    #[test]
    fn returns_405_when_wrong_method() {
        start(|| {
            let routes = Router::new()
                .route(Method::Get, "/", |r: Responder| r.send(()))
                .route(Method::Options, "/", |r: Responder| r.send(()));
            let mut client = FakeClient::from(routes);

            let response = client.post("/").send(());

            assert_eq!(response.status, StatusCode::MethodNotAllowed);
            let (_, allow) = response
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("allow"))
                .unwrap();
            dbg!(String::from_utf8_lossy(allow));
            assert_eq!(String::from_utf8_lossy(allow), "GET, HEAD, OPTIONS");
        })
        .unwrap();
    }

    #[test]
    #[ignore]
    fn head_defers_to_get() {
        start(|| {
            let routes = Router::new().route(Method::Get, "/", |r: Responder| r.send("hello"));
            let mut client = FakeClient::from(routes);

            let response = client.head("/").send(());

            assert_eq!(response.status, StatusCode::Ok);
            assert!(response.body.is_empty());
        })
        .unwrap();
    }

    #[test]
    fn custom_head_overrides_get() {
        start(|| {
            let routes = Router::new()
                .route(Method::Get, "/", |r: Responder| {
                    r.status(StatusCode::Forbidden).send(())
                })
                .route(Method::Head, "/", |r: Responder| {
                    r.status(StatusCode::Accepted).send(())
                });
            let mut client = FakeClient::from(routes);

            let response = client.head("/").send(());

            assert_eq!(response.status, StatusCode::Accepted);
        })
        .unwrap();
    }

    #[test]
    fn handles_state() {
        start(|| {
            let routes = Router::new().route(Method::Get, "/", |r: Responder, state: &String| {
                r.send(state.as_str())
            });
            let mut client = FakeClient::new(routes, String::from("hello"));

            let response = client.get("/").send(());

            assert_eq!(response.status, StatusCode::Ok);
            assert_eq!(response.body, b"hello");
        })
        .unwrap();
    }
}
