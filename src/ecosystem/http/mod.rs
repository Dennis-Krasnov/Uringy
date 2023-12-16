//! Opinionated HTTP 1.1 client and server inspired by Axum.
//!
//! Unlike Axum, this library is zero copy (better performance).
//! This means the request/response can reference the stack and the server can reference the request.
//!
//! The major drawback of this design is that streaming isn't supported.
//! This means requests and responses must fit into memory (you can configure the buffer size).
//! There is no support for chunked transfer encoding nor SSE.
//!
//! The justification is that most clients/servers don't need streaming:
//! - Typically there's a streaming reverse proxy in front of your server that buffers requests/responses and sends/receives them at once.
//! - It's pure overhead for small to medium payloads.
//! - It's too naive for big payloads, where you would manually chunk for the ability to resume a partially completed transfer.
//! - Realtime applications outside the web aren't limited by HTTP and can use better suiting protocols like gRPC.
//! - Realtime applications on the web still have the option of long polling, websockets, and eventually WebTransport.
//! - If you still need streaming, you can proxy that endpoint to a server that supports it.
//!
//! There are plans to support websockets and connect tunnels, as they respond like normal then hijack the whole connection.
//!
//! HTTP 2/3 aren't supported since they aren't compatible with the zero copy design.
//! Use a reverse proxy like Nginx to support these newer protocols, remember to enable keepalive to the origin.

use crate::ecosystem::http::payload::Request;

pub mod client;
pub mod payload;
pub mod server;

pub mod middleware;
pub mod mime;
// mod macros;

/// Dynamically dispatched handle to the next step in processing the request.
pub type Handler = Box<dyn Fn(Responder, &Request)>;

/// Dynamically dispatched handle to the next step in processing the response.
pub struct Responder(Box<dyn Respond>);

impl Responder {
    /// ...
    pub fn send(self, response: impl payload::IntoResponse) {
        self.0.respond(response.into_response());
    }
}

/// A concrete `Responder::send` is exposed instead of this trait because:
/// - You don't need to import the `Respond` trait to send responses.
/// - It allows you to take non-object safe `impl IntoResponse`.
trait Respond {
    fn respond(self: Box<Self>, response: payload::Response);
}

/// ...
///
/// Generic `ARGS` prevent conflicting implementations.
pub trait IntoHandler<ARGS> {
    /// ...
    fn into_handler(self) -> Handler;
}

impl<F: Fn(Responder) + 'static> IntoHandler<(Responder,)> for F {
    fn into_handler(self) -> Handler {
        Box::new(move |r, _| self(r))
    }
}

impl<F: Fn(Responder, &Request) + 'static> IntoHandler<(Responder, &Request<'_>)> for F {
    fn into_handler(self) -> Handler {
        Box::new(move |r, request| self(r, request))
    }
}
