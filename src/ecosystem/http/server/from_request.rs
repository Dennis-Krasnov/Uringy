//! ... extract...

use std::convert::Infallible;
use std::io::Read;

use http::{HeaderMap, Method, StatusCode, Uri, Version};
use serde::de::DeserializeOwned;

use crate::ecosystem::http::request::Parts;
use crate::ecosystem::http::server::into_response::IntoResponse;
use crate::ecosystem::http::server::macros;
use crate::ecosystem::http::Request;

/// Types that can be created from an entire [Request].
///
/// Generic over [M] to allow specifying a unique type to avoid conflicting implementations.
pub trait FromRequest<M = ()>: Sized {
    /// ...
    type Rejection: IntoResponse; // TODO: standardize erroneous type name

    /// ...
    fn from_request(request: Request) -> Result<Self, Self::Rejection>;
}

impl FromRequest for Request {
    type Rejection = Infallible;

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        Ok(request)
    }
}

impl FromRequest for Parts {
    type Rejection = Infallible;

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        Ok(request.into_parts().0)
    }
}

impl FromRequest for String {
    type Rejection = Infallible;

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        Ok(request.into_string().unwrap())
    }
}

impl FromRequest for Vec<u8> {
    type Rejection = Infallible;

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        Ok(request.into_vec().unwrap())
    }
}

impl FromRequest for Box<dyn Read> {
    type Rejection = Infallible;

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        Ok(request.into_body())
    }
}

impl<T: FromRequest> FromRequest for Option<T> {
    type Rejection = Infallible;

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(request).ok())
    }
}

impl<T: FromRequest> FromRequest for Result<T, T::Rejection> {
    type Rejection = Infallible;

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        Ok(T::from_request(request))
    }
}

macro_rules! impl_from_request {
    (
        [$($ty:ident),*], $last:ident
    ) => {
        impl<$($ty,)* $last> FromRequest for ($($ty,)* $last,)
        where
            $( $ty: FromRequestParts, )*
            $last: FromRequest,
        {
            type Rejection = $crate::ecosystem::http::Response;

            fn from_request(_request: $crate::ecosystem::http::Request) -> Result<Self, Self::Rejection> {

                // let (mut parts, body) = req.into_parts();
                //
                //                 $(
                //                     let $ty = $ty::from_request_parts(&mut parts, state).await.map_err(|err| err.into_response())?;
                //                 )*
                //
                //                 let req = Request::from_parts(parts, body);
                //
                // let $last = $last::from_request(request).map_err(|err| err.into_response())?;
                //
                //                 Ok(($($ty,)* $last,))
                todo!()
            }
        }
    };
}

macros::all_the_tuples_and_last!(impl_from_request);

/// Types that can be created from request [Parts].
pub trait FromRequestParts: Sized {
    /// Error that can be converted into a response.
    type Rejection: IntoResponse;

    /// Perform the extraction.
    fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection>;
}

impl FromRequestParts for Method {
    type Rejection = Infallible;

    fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(parts.method.clone())
    }
}

impl FromRequestParts for Uri {
    type Rejection = Infallible;

    fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(parts.uri.clone())
    }
}

impl FromRequestParts for Version {
    type Rejection = Infallible;

    fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(parts.version.clone())
    }
}

impl FromRequestParts for HeaderMap {
    type Rejection = Infallible;

    fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(parts.headers.clone())
    }
}

/// ...
impl<P: FromRequestParts> FromRequest<P> for P {
    type Rejection = <Self as FromRequestParts>::Rejection;

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        let (mut parts, _) = request.into_parts();
        Self::from_request_parts(&mut parts)
    }
}

impl<T: FromRequestParts> FromRequestParts for Option<T> {
    type Rejection = Infallible;

    fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(T::from_request_parts(parts).ok())
    }
}

impl<T: FromRequestParts> FromRequestParts for Result<T, T::Rejection> {
    type Rejection = Infallible;

    fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        Ok(T::from_request_parts(parts))
    }
}

macro_rules! impl_from_request_parts {
    (
        [$($ty:ident),*], $last:ident
    ) => {

        impl<$($ty,)* $last> FromRequestParts for ($($ty,)* $last,)
        where
            $( $ty: FromRequestParts, )*
            $last: FromRequestParts,
        {
            type Rejection = $crate::ecosystem::http::Response;

            fn from_request_parts(_parts: &mut Parts) -> Result<Self, Self::Rejection> {
                // $(
                //                     let $ty = $ty::from_request_parts(parts, state)
                //                         .await
                //                         .map_err(|err| err.into_response())?;
                //                 )*
                //                 let $last = $last::from_request_parts(parts, state)
                //                     .await
                //                     .map_err(|err| err.into_response())?;
                //
                //                 Ok(($($ty,)* $last,))
                todo!()
            }
        }
    };
}

macros::all_the_tuples_and_last!(impl_from_request_parts);

/// Extractor that deserializes query strings into a deserializable type.
///
/// `400 Bad Request` is returned if the query string can't be parsed.
pub struct Query<T>(pub T);

impl<T: DeserializeOwned> FromRequestParts for Query<T> {
    type Rejection = StatusCode;

    fn from_request_parts(parts: &mut Parts) -> Result<Self, Self::Rejection> {
        let query = parts.uri.query().unwrap_or_default();
        let value = serde_urlencoded::from_str(query).map_err(|_| StatusCode::BAD_REQUEST)?;
        Ok(Query(value))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde::Deserialize;

    use crate::ecosystem::http::server::routing::{get, Router};
    use crate::ecosystem::http::Request;

    use super::*;

    mod query {
        use super::*;

        #[test]
        fn takes_valid() {
            fn root(Query(params): Query<HashMap<String, String>>) {
                assert_eq!(params["roses"], "red");
                assert_eq!(params["violets"], "blue");
            }
            let router = Router::new().route("/", get(root));

            let request = Request::get("/?roses=red&violets=blue").body(()).unwrap();
            let response = router.handle(request);

            assert_eq!(response.status(), &StatusCode::OK);
        }

        #[test]
        fn cant_take_invalid_value() {
            #[derive(Deserialize)]
            struct Params {
                _number: i32,
            }
            fn root(_: Query<Params>) {
                unreachable!();
            }
            let router = Router::new().route("/", get(root));

            let request = Request::get("/").body(()).unwrap();
            let response = router.handle(request);

            assert_eq!(response.status(), &StatusCode::BAD_REQUEST);
        }
    }

    #[test]
    fn compiles() {
        Router::new()
            // single argument
            .route("/no_args", get(|| {}))
            .route("/request", get(|_: Request| {}))
            .route("/optional-request", get(|_: Option<Request>| {}))
            .route("/result-request", get(|_: Result<Request, _>| {}))
            .route("/nested-request", get(|_: ((Request,),)| {}))
            .route("/part", get(|_: Query<()>| {}))
            .route("/optional-part", get(|_: Option<Query<()>>| {}))
            .route("/result-part", get(|_: Result<Query<()>, _>| {}))
            .route("/nested-request-parts", get(|_: ((Query<()>,),)| {}))
            .route("/method", get(|_: Method| {}))
            .route("/uri", get(|_: Uri| {}))
            .route("/version", get(|_: Version| {}))
            .route("/headers", get(|_: HeaderMap| {}))
            .route("/parts", get(|_: Parts| {}))
            .route("/string", get(|_: String| {}))
            .route("/vec", get(|_: Vec<u8>| {}))
            .route("/reader", get(|_: Box<dyn Read>| {}))
            // multiple arguments
            .route("/part-part", get(|_: Query<()>, _: Query<()>| {}))
            .route("/part-request", get(|_: Query<()>, _: Request| {}))
            .route(
                "/part-part-request",
                get(|_: Query<()>, _: Query<()>, _: Request| {}),
            );
    }
}
