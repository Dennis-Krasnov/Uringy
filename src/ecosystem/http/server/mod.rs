//! ...

use std::io;
use std::io::{Read, Write};

use crate::circular_buffer::CircularBuffer;
use crate::ecosystem::http::server::into_response::IntoResponse;
use crate::ecosystem::http::{Request, Response};

pub mod from_request;
pub mod into_response;
mod macros;
pub mod routing;
// TODO: pub mod middleware

/// ...
pub type Service = Box<dyn Fn(Request) -> Response>;

/// ...
pub trait Handler<ARGS>: Clone {
    /// ...
    fn call(self, request: Request) -> Response;
}

impl<F: FnOnce() -> R + Clone, R: IntoResponse<M>, M> Handler<((), M)> for F {
    fn call(self, _: Request) -> Response {
        self().into_response()
    }
}

macro_rules! impl_handler {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        // include M and variadic arguments in Handler's ARGS to constrain generic type parameters
        #[allow(non_snake_case, unused_mut)]
        impl<F, R, M, M2, $($ty,)* $last> Handler<(M, M2, $($ty,)* $last,)> for F
        where
            F: FnOnce($($ty,)* $last,) -> R + Clone,
            R: $crate::ecosystem::http::server::into_response::IntoResponse<M2>,
            $( $ty: $crate::ecosystem::http::server::from_request::FromRequestParts, )*
            $last: $crate::ecosystem::http::server::from_request::FromRequest<M>,
        {
            fn call(self, request: $crate::ecosystem::http::Request) -> $crate::ecosystem::http::Response {
                let (mut parts, body) = request.into_parts();

                $(
                    let $ty = match $ty::from_request_parts(&mut parts) {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };
                )*

                let request = $crate::ecosystem::http::Request::from_parts(parts, body);

                let $last = match $last::from_request(request) {
                    Ok(value) => value,
                    Err(rejection) => return rejection.into_response(),
                };

                self($($ty,)* $last,).into_response()
            }
        }
    };
}

macros::all_the_tuples_and_last!(impl_handler);

fn serialize(mut writer: impl Write, response: Response) -> crate::IoResult<()> {
    writer.write_all(format!("{:?}", response.version()).as_bytes())?;
    writer.write_all(b" ")?;
    writer.write_all(response.status().as_str().as_bytes())?;
    writer.write_all(b" ")?;
    writer.write_all(response.status().canonical_reason().unwrap().as_bytes())?;
    writer.write_all(b"\r\n")?;

    for (name, value) in response.headers() {
        writer.write_all(name.as_str().as_bytes())?;
        writer.write_all(b": ")?;
        writer.write_all(value.as_bytes())?;
        writer.write_all(b"\r\n")?;
    }
    writer.write_all(b"\r\n")?;

    let mut body = response.into_body();
    io::copy(&mut body, &mut writer).map_err(crate::Error::from_io_error)?;

    Ok(())
}

fn deserialize(mut reader: impl Read) -> crate::IoResult<Request> {
    let mut buffer = CircularBuffer::new(4096)?;

    loop {
        let bytes_read = reader.read(&mut buffer.uninit())?;
        buffer.commit(bytes_read);

        if bytes_read == 0 {
            panic!("oops"); // TODO: return correct Err
        }

        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut headers);

        match request.parse(&buffer.data()) {
            Ok(httparse::Status::Complete(wire_size)) => {
                let mut builder = Request::builder()
                    .method(request.method.unwrap())
                    .uri(request.path.unwrap())
                    .version(http::Version::HTTP_11); // TODO: request.version.unwrap()

                let body_size: usize = request
                    .headers
                    .iter()
                    .find(|h| h.name.to_ascii_lowercase() == "content-length")
                    .and_then(|h| std::str::from_utf8(h.value).ok())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);

                for httparse::Header { name, value } in request.headers {
                    builder = builder.header(name.to_string(), value.to_vec());
                }

                if buffer.data().len() < wire_size + body_size {
                    println!("server reading more!");
                    continue;
                }

                let body = buffer.data()[wire_size..(wire_size + body_size)].to_vec();

                buffer.consume(wire_size); // copy from buffer before consuming
                break Ok(builder.raw_body(body).unwrap());
            }
            Ok(httparse::Status::Partial) => continue,
            Err(e) => panic!("oops: {e}"),
        }
    }
}
