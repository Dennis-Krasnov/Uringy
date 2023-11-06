//! body, extractor...

use std::io::{Cursor, Read};
use std::str::FromStr;

use mime::Mime;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::ecosystem::http::into_body::IntoBody;
use crate::ecosystem::http::server::from_request::FromRequest;
use crate::ecosystem::http::{header, Request, StatusCode};

/// ...
pub struct Json<T>(pub T);

impl<T: Serialize> IntoBody for Json<T> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        let buffer = serde_json::to_vec(&self.0).unwrap(); // FIXME: exception handling
        let content_length = Some(buffer.len());
        let body = Box::new(Cursor::new(buffer));
        (content_length, body)
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::APPLICATION_JSON)
    }
}

impl<T: DeserializeOwned> FromRequest for Json<T> {
    type Rejection = (StatusCode, &'static str);

    fn from_request(request: Request) -> Result<Self, Self::Rejection> {
        let is_json = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| Mime::from_str(h).ok())
            .map_or(false, |mime| {
                mime.type_() == "application"
                    && (mime.subtype() == "json"
                        || mime.suffix().map_or(false, |name| name == "json"))
            });

        if !is_json {
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Expected request with `Content-Type: application/json`",
            ));
        }

        let body = request.into_body();
        let value = serde_json::from_reader(body).map_err(|e| match e.classify() {
            serde_json::error::Category::Io => (StatusCode::INTERNAL_SERVER_ERROR, ""),
            serde_json::error::Category::Syntax | serde_json::error::Category::Eof => (
                StatusCode::BAD_REQUEST,
                "Failed to parse the request body as JSON`",
            ),
            serde_json::error::Category::Data => (
                StatusCode::BAD_REQUEST,
                "Failed to deserialize the JSON body into the target type",
            ),
        })?;
        Ok(Json(value))
    }
}

/// ...
pub struct Html<T>(pub T);

impl<T: IntoBody> IntoBody for Html<T> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        self.0.into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::TEXT_HTML_UTF_8)
    }
}

/// ...
pub struct JavaScript<T>(pub T);

impl<T: IntoBody> IntoBody for JavaScript<T> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        self.0.into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::APPLICATION_JAVASCRIPT_UTF_8)
    }
}

/// ...
pub struct Css<T>(pub T);

impl<T: IntoBody> IntoBody for Css<T> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        self.0.into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::TEXT_CSS_UTF_8)
    }
}

#[cfg(test)]
mod tests {
    use crate::ecosystem::http::server::routing::{get, Router};
    use crate::ecosystem::http::{Request, StatusCode};

    use super::*;

    mod json {
        use super::*;

        #[test]
        fn takes_valid() {
            fn root(Json(number): Json<i32>) {
                assert_eq!(number, 42);
            }
            let router = Router::new().route("/", get(root));

            let request = Request::get("/").body(Json(42)).unwrap();
            let response = router.handle(request);

            assert_eq!(response.status(), &StatusCode::OK);
        }

        #[test]
        fn cant_take_when_missing_content_type() {
            fn root(_: Json<i32>) {
                unreachable!();
            }
            let router = Router::new().route("/", get(root));

            let request = Request::get("/").body(()).unwrap();
            let response = router.handle(request);

            assert_eq!(response.status(), &StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[test]
        fn cant_take_when_invalid_content_type() {
            fn root(_: Json<i32>) {
                unreachable!();
            }
            let router = Router::new().route("/", get(root));

            let request = Request::get("/").body("42").unwrap();
            let response = router.handle(request);

            assert_eq!(response.status(), &StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[test]
        fn cant_take_when_invalid_value() {
            fn root(_: Json<i32>) {
                unreachable!();
            }
            let router = Router::new().route("/", get(root));

            let request = Request::get("/").body(Json("42")).unwrap();
            let response = router.handle(request);

            assert_eq!(response.status(), &StatusCode::BAD_REQUEST);
        }
    }

    #[test]
    fn is_into_body() {
        Request::builder().body(Json("hello")).unwrap();
        Request::builder().body(Html("hello")).unwrap();
        Request::builder().body(JavaScript("hello")).unwrap();
        Request::builder().body(Css("hello")).unwrap();
    }
}
