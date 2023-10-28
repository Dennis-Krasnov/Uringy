//! ...

pub use http::header;
pub use http::Extensions;
pub use http::Method;
pub use http::StatusCode;
pub use http::Uri;
pub use http::Version;

pub use request::Request;
pub use response::Response;

pub mod client;
pub mod into_body;
pub mod mime;
pub mod request;
pub mod response;
pub mod server;

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use crate::ecosystem::http::server::into_response::IntoResponse;
    use crate::ecosystem::http::server::routing::{get, Router};
    use crate::ecosystem::http::{client, Request};
    use crate::net::tcp;
    use crate::runtime::{spawn, start};

    #[test]
    fn end_to_end() {
        start(|| {
            let listener = tcp::Listener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
            let port = listener.local_addr().unwrap().port();

            let server = spawn(move || {
                let connection = listener.accept().unwrap().0;
                let router = Router::new().route("/", get(root));
                router.serve(connection).unwrap();
            });

            let connection = tcp::Stream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
            let request = Request::get("/").body("hi").unwrap();
            let response = client::issue(connection, request).unwrap();
            dbg!(&response);
            assert_eq!(response.into_string().unwrap(), "hello");

            server.join().unwrap();
        })
        .unwrap();

        fn root(body: String) -> impl IntoResponse {
            assert_eq!(body, "hi");
            "hello"
        }
    }
}
