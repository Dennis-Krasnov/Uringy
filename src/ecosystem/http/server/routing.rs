//! ...

use std::io::{Read, Write};

use crate::ecosystem::http::server::into_response::IntoResponse;
use crate::ecosystem::http::server::Handler;

/// Handle for composing endpoint handlers.
pub struct Router {
    matcher: matchit::Router<super::Service>,
}

impl Router {
    /// ...
    pub fn new() -> Self {
        Router {
            matcher: matchit::Router::new(),
        }
    }

    /// Add a route to the router.
    pub fn route(mut self, path: &str, method_router: MethodRouter) -> Self {
        // TODO: struct with a field for each method, on handle match request's method
        self.matcher.insert(path, method_router.get).unwrap();
        self
    }

    /// ...
    pub fn handle(&self, request: super::Request) -> super::Response {
        let Ok(endpoint) = self.matcher.at(request.uri().path()) else {
            return http::StatusCode::NOT_FOUND.into_response();
        };

        // endpoint.get("id")

        (endpoint.value)(request)
    }

    /// ...
    pub fn serve(&self, mut connection: impl Read + Write) -> crate::IoResult<()> {
        let request = super::deserialize(&mut connection)?;

        let response = self.handle(request);

        let mut writer = std::io::BufWriter::new(connection);
        super::serialize(&mut writer, response)?;
        writer.flush()?;

        Ok(())
    }
}

/// ...
// TODO: fallback routes
pub struct MethodRouter {
    get: super::Service,
    // post: super::Service,
}

/// Route GET requests to the given handler.
pub fn get<ARGS>(handler: impl Handler<ARGS> + 'static) -> MethodRouter {
    MethodRouter {
        get: Box::new(move |request| handler.clone().call(request)),
    }
}
