//! ...
//!
//! # Supported...
//! Response
//! status_code
//! into_parts
//! into_body
//! Result
//! (status_code, into_parts, into_body)
//!     0..1        0..15     0..1

use std::convert::Infallible;
use std::time::SystemTime;

use http::StatusCode;

use crate::ecosystem::http::into_body::IntoBody;
use crate::ecosystem::http::server::macros;

use super::Response;

/// Trait for generating responses.
///
/// Generic over [M] to allow specifying a unique type to avoid conflicting implementations.
pub trait IntoResponse<M = ()> {
    /// Create a response.
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

/// ...
impl IntoResponse for Infallible {
    fn into_response(self) -> Response {
        match self {}
    }
}

impl IntoResponse for http::StatusCode {
    fn into_response(self) -> Response {
        let mut response = ().into_response();
        *response.status_mut() = self;
        response
    }
}

/// Tuples of [IntoResponseParts] collapse into a single [IntoResponseParts].
impl<P: IntoResponseParts> IntoResponse<P> for P {
    fn into_response(self) -> Response {
        let response = ().into_response();
        let parts = ResponseParts(response);

        let parts = match self.into_response_parts(parts) {
            Ok(parts) => parts,
            Err(err) => return err.into_response(),
        };

        parts.0
    }
}

impl<B: IntoBody> IntoResponse for B {
    fn into_response(self) -> Response {
        Response::builder()
            .header(
                http::header::DATE,
                httpdate::fmt_http_date(SystemTime::now()),
            )
            .header(
                http::header::SERVER,
                http::HeaderValue::from_static("Uringy"),
            )
            .body(self)
            .unwrap()
    }
}

impl<T, E, M1, M2> IntoResponse<(M1, M2)> for Result<T, E>
where
    T: IntoResponse<M1>,
    E: IntoResponse<M2>,
{
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

macro_rules! impl_into_response {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<$($ty,)*> IntoResponse<($($ty,)*)> for ($crate::ecosystem::http::StatusCode, $($ty),*)
        where
            $( $ty: IntoResponseParts, )*
        {
            fn into_response(self) -> $crate::ecosystem::http::Response {
                let (status, $($ty),*) = self;

                let response = status.into_response();
                let parts = ResponseParts(response);

                $(
                    let parts = match $ty.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => return err.into_response(),
                    };
                )*

                parts.0
            }
        }

        #[allow(non_snake_case)]
        impl<B, $($ty,)*> IntoResponse<($($ty,)*)> for ($($ty),*, B)
        where
            $( $ty: IntoResponseParts, )*
            B: IntoBody,
        {
            fn into_response(self) -> $crate::ecosystem::http::Response {
                let ($($ty),*, body) = self;

                let response = body.into_response();
                let parts = ResponseParts(response);

                $(
                    let parts = match $ty.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => return err.into_response(),
                    };
                )*

                parts.0
            }
        }

        #[allow(non_snake_case)]
        impl<B, $($ty,)*> IntoResponse<($($ty,)*)> for ($crate::ecosystem::http::StatusCode, $($ty),*, B)
        where
            $( $ty: IntoResponseParts, )*
            B: IntoBody,
        {
            fn into_response(self) -> $crate::ecosystem::http::Response {
                let (status, $($ty),*, body) = self;

                let response = (status, body).into_response();
                let parts = ResponseParts(response);

                $(
                    let parts = match $ty.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => return err.into_response(),
                    };
                )*

                parts.0
            }
        }
    };
}

macros::all_the_tuples!(impl_into_response);

/// Case isn't handled by macro.
impl IntoResponse for (http::StatusCode,) {
    fn into_response(self) -> Response {
        let mut response = ().into_response();
        *response.status_mut() = self.0;
        response
    }
}

/// Case isn't handled by macro.
impl<B: IntoBody> IntoResponse for (B,) {
    fn into_response(self) -> Response {
        self.0.into_response()
    }
}

/// Case isn't handled by macro.
impl<B: IntoBody> IntoResponse for (http::StatusCode, B) {
    fn into_response(self) -> Response {
        let mut response = self.1.into_response();
        *response.status_mut() = self.0;
        response
    }
}

/// ...
#[derive(Debug)]
pub struct ResponseParts(Response);

impl ResponseParts {
    /// Gets a reference to the response headers.
    pub fn headers(&self) -> &http::HeaderMap {
        self.0.headers()
    }

    /// Gets a mutable reference to the response headers.
    pub fn headers_mut(&mut self) -> &mut http::HeaderMap {
        self.0.headers_mut()
    }

    // /// Gets a reference to the response extensions.
    // pub fn extensions(&self) -> &http::Extensions {
    //     self.0.extensions()
    // }
    //
    // /// Gets a mutable reference to the response extensions.
    // pub fn extensions_mut(&mut self) -> &mut http::Extensions {
    //     self.0.extensions_mut()
    // }
}

// TODO: headers, headers_mut, extensions, extensions_mut

/// Trait for adding headers and extensions to a response.
pub trait IntoResponseParts {
    /// The type returned in the event of an error.
    ///
    /// This can be used to fallibly convert types into headers or extensions.
    type Error: IntoResponse;

    /// Set parts of the response
    fn into_response_parts(self, response: ResponseParts) -> Result<ResponseParts, Self::Error>;
}

impl<T: IntoResponseParts> IntoResponseParts for Option<T> {
    type Error = T::Error;

    fn into_response_parts(self, response: ResponseParts) -> Result<ResponseParts, Self::Error> {
        if let Some(inner) = self {
            inner.into_response_parts(response)
        } else {
            Ok(response)
        }
    }
}

impl IntoResponseParts for http::HeaderMap {
    type Error = Infallible;

    fn into_response_parts(
        self,
        mut response: ResponseParts,
    ) -> Result<ResponseParts, Self::Error> {
        response.headers_mut().extend(self);
        Ok(response)
    }
}

impl<K, V, const N: usize> IntoResponseParts for [(K, V); N]
where
    K: TryInto<http::HeaderName>,
    K::Error: Into<http::Error>,
    V: TryInto<http::HeaderValue>,
    V::Error: Into<http::Error>,
{
    type Error = (StatusCode, &'static str);

    fn into_response_parts(
        self,
        mut response: ResponseParts,
    ) -> Result<ResponseParts, Self::Error> {
        for (key, value) in self {
            let key = key.try_into().map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to convert key to a header name",
                )
            })?;
            let value = value.try_into().map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to convert value to a header value",
                )
            })?;
            response.headers_mut().insert(key, value);
        }

        Ok(response)
    }
}
macro_rules! impl_into_response_parts {
    ( $($ty:ident),* $(,)? ) => {
        #[allow(non_snake_case)]
        impl<$($ty,)*> IntoResponseParts for ($($ty,)*)
        where
            $( $ty: IntoResponseParts, )*
        {
            type Error = $crate::ecosystem::http::Response;

            fn into_response_parts(self, response: $crate::ecosystem::http::server::into_response::ResponseParts) -> Result<$crate::ecosystem::http::server::into_response::ResponseParts, Self::Error> {
                let ($($ty,)*) = self;

                $(
                    let response = match $ty.into_response_parts(response) {
                        Ok(res) => res,
                        Err(err) => return Err(err.into_response()),
                    };
                )*

                Ok(response)
            }
        }
    }
}

macros::all_the_tuples!(impl_into_response_parts);

#[cfg(test)]
mod tests {
    use http::{HeaderMap, StatusCode};

    use crate::ecosystem::http::server::routing::{get, Router};
    use crate::ecosystem::http::Response;

    // #[test]
    // fn returns_into_body() {
    //     let router = Router::new().route("/", get(|| ()));
    //
    //     let request = Request::get("/").body(()).unwrap();
    //     let response = router.handle(request);
    //
    //     assert_eq!(response.status(), &StatusCode::OK);
    //     assert!(response.headers().contains_key(http::header::DATE));
    //     assert_eq!(response.headers()[http::header::SERVER], "Uringy");
    //     assert!(response.into_string().unwrap().is_empty());
    // }
    //
    // #[test]
    // fn returns_status_code() {
    //     let router = Router::new().route("/", get(|| StatusCode::ACCEPTED));
    //
    //     let request = Request::get("/").body(()).unwrap();
    //     let response = router.handle(request);
    //
    //     assert_eq!(response.status(), &StatusCode::ACCEPTED);
    //     assert!(response.headers().contains_key(http::header::DATE));
    //     assert_eq!(response.headers()[http::header::SERVER], "Uringy");
    //     assert!(response.into_string().unwrap().is_empty());
    // }
    //
    // #[test]
    // fn returns_status_code_and_body() {
    //     let router = Router::new().route("/", get(|| (StatusCode::ACCEPTED, b"hi")));
    //
    //     let request = Request::get("/").body(()).unwrap();
    //     let response = router.handle(request);
    //
    //     assert_eq!(response.status(), &StatusCode::ACCEPTED);
    //     assert!(response.headers().contains_key(http::header::DATE));
    //     assert_eq!(response.headers()[http::header::SERVER], "Uringy");
    //     assert_eq!(response.into_vec().unwrap(), b"hi");
    // }

    #[test]
    fn compiles() {
        Router::new()
            // single value
            .route("/response", get(|| Response::builder().body(()).unwrap()))
            .route("/code", get(|| StatusCode::ACCEPTED))
            .route("/response-part", get(|| HeaderMap::new()))
            .route("/body", get(|| "hi"))
            .route("/result", get(|| Ok::<_, String>("hi")))
            // single value tuple
            .route("/tuple-code", get(|| (StatusCode::ACCEPTED,)))
            .route("/tuple-response-part", get(|| (HeaderMap::new(),)))
            .route("/tuple-body", get(|| ("hi",)))
            // two value tuple
            .route(
                "/tuple-code-part",
                get(|| (StatusCode::ACCEPTED, HeaderMap::new())),
            )
            .route("/tuple-code-body", get(|| (StatusCode::ACCEPTED, "hi")))
            .route(
                "/tuple-part-part",
                get(|| (HeaderMap::new(), HeaderMap::new())),
            )
            .route("/tuple-part-body", get(|| (HeaderMap::new(), "hi")))
            // 3+ value tuple
            .route(
                "/tuple-code-part-part",
                get(|| (StatusCode::ACCEPTED, HeaderMap::new(), HeaderMap::new())),
            )
            .route(
                "/tuple-code-part-body",
                get(|| (StatusCode::ACCEPTED, HeaderMap::new(), "hi")),
            )
            .route(
                "/tuple-code-part-part-body",
                get(|| {
                    (
                        StatusCode::ACCEPTED,
                        HeaderMap::new(),
                        HeaderMap::new(),
                        "hi",
                    )
                }),
            )
            .route(
                "/tuple-part-part-body",
                get(|| (HeaderMap::new(), HeaderMap::new(), "hi")),
            )
            // response parts
            .route(
                "/nested-response-parts",
                get(|| (HeaderMap::new(), (HeaderMap::new(), HeaderMap::new()))),
            )
            .route("/optional-response-part", get(|| Some(HeaderMap::new())))
            .route("/header-list", get(|| [("x-foo", "bar")]));
    }
}
