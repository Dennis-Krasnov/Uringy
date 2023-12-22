//! ...

use crate::ecosystem::http::payload::IntoBody;

/// ...
pub struct Html<T>(pub T);

impl<T: IntoBody> IntoBody for Html<T> {
    fn contents(&self) -> &[u8] {
        self.0.contents()
    }

    fn content_type(&self) -> Option<&str> {
        Some("text/html; charset=utf-8")
    }
}

/// ...
pub struct JavaScript<T>(pub T);

impl<T: IntoBody> IntoBody for JavaScript<T> {
    fn contents(&self) -> &[u8] {
        self.0.contents()
    }

    fn content_type(&self) -> Option<&str> {
        Some("text/javascript")
    }
}

/// ...
pub struct Css<T>(pub T);

impl<T: IntoBody> IntoBody for Css<T> {
    fn contents(&self) -> &[u8] {
        self.0.contents()
    }

    fn content_type(&self) -> Option<&str> {
        Some("text/css")
    }
}

/// ...
pub struct Png<T>(pub T);

impl<T: IntoBody> IntoBody for Png<T> {
    fn contents(&self) -> &[u8] {
        self.0.contents()
    }

    fn content_type(&self) -> Option<&str> {
        Some("image/png")
    }
}

/// ...
pub struct Woff2<T>(pub T);

impl<T: IntoBody> IntoBody for Woff2<T> {
    fn contents(&self) -> &[u8] {
        self.0.contents()
    }

    fn content_type(&self) -> Option<&str> {
        Some("font/woff2")
    }
}

// https://docs.rs/mime/latest/src/mime/lib.rs.html#746-784
