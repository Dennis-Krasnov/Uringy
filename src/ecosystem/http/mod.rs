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

use crate::ecosystem::http::payload::{Request, Response, StatusCode};
use std::marker::PhantomData;

pub mod client;
pub mod payload;
pub mod server;

pub mod middleware;
pub mod mime;

/// Dynamically dispatched handle to the next step in processing the request.
pub type Handler<S> = Box<dyn Fn(Responder, &Request, &S)>;

/// Dynamically dispatched handle to the next step in processing the response.
pub struct Responder<'a, TS = DefaultStatusCode> {
    respond: Box<dyn Respond>,
    type_state: PhantomData<TS>,
    status: StatusCode,
    headers: Vec<(&'a str, &'a [u8])>,
}

/// Type state for `Responder`.
pub struct DefaultStatusCode;
pub struct CustomStatusCode;

impl<'a> Responder<'a, DefaultStatusCode> {
    fn new(respond: impl Respond + 'static) -> Self {
        Responder {
            respond: Box::new(respond),
            type_state: PhantomData,
            status: StatusCode::Ok,
            headers: vec![],
        }
    }

    /// ...
    #[inline]
    pub fn status(self, status: StatusCode) -> Responder<'a, CustomStatusCode> {
        Responder {
            respond: self.respond,
            type_state: PhantomData,
            status,
            headers: self.headers,
        }
    }
}

impl<'a, TS> Responder<'a, TS> {
    /// ...
    #[inline]
    pub fn header(mut self, name: &'a str, value: &'a [u8]) -> Self {
        self.headers.push((name, value));
        self
    }

    /// ...
    #[inline]
    pub fn send(self, body: impl payload::AsBody) {
        let response = Response {
            status: self.status,
            headers: self.headers,
            body: body.contents(),
            content_type: body.content_type(),
        };
        self.respond.respond(response);
    }
}

/// A concrete `Responder::send` is exposed instead of this trait because:
/// - You don't need to import the `Respond` trait to send responses.
/// - It allows you to take non-object safe `impl IntoResponse`.
trait Respond {
    fn respond(self: Box<Self>, response: Response);
}

/// ...
///
/// Generic `ARGS` prevent conflicting implementations.
pub trait IntoHandler<ARGS, S> {
    /// ...
    fn into_handler(self) -> Handler<S>;
}

impl<F: Fn(Responder) + 'static, S> IntoHandler<(Responder<'_>,), S> for F {
    fn into_handler(self) -> Handler<S> {
        Box::new(move |r, _, _| self(r))
    }
}

impl<F: Fn(Responder, &S) + 'static, S> IntoHandler<(Responder<'_>, (), &S), S> for F {
    fn into_handler(self) -> Handler<S> {
        Box::new(move |r, _, state| self(r, state))
    }
}

impl<F: Fn(Responder, &Request) + 'static, S> IntoHandler<(Responder<'_>, &Request<'_>, ()), S>
    for F
{
    fn into_handler(self) -> Handler<S> {
        Box::new(move |r, request, _| self(r, request))
    }
}

impl<F: Fn(Responder, &Request, &S) + 'static, S> IntoHandler<(Responder<'_>, &Request<'_>, &S), S>
    for F
{
    fn into_handler(self) -> Handler<S> {
        Box::new(move |r, request, state| self(r, request, state))
    }
}
