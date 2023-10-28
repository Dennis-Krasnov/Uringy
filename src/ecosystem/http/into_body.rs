//! ... request and response.

use std::borrow::Cow;
use std::io::{Cursor, Read};

use mime::Mime;

/// Trait for generating bodies.
pub trait IntoBody {
    // TODO: decide whether this is infallible or not

    /// Create a response.
    /// ... for Content-Length header
    fn into_body(self) -> (Option<usize>, Box<dyn Read>);

    /// ... for Content-Type header
    fn content_type(&self) -> Option<Mime>;
}

impl IntoBody for () {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        let content_length = Some(0);
        let body = Box::new(Cursor::new(vec![]));
        (content_length, body)
    }

    fn content_type(&self) -> Option<Mime> {
        None
    }
}

impl IntoBody for &'static str {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        Cow::Borrowed(self).into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::TEXT_PLAIN_UTF_8)
    }
}

impl IntoBody for String {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        Cow::<'static, str>::Owned(self).into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::TEXT_PLAIN_UTF_8)
    }
}

impl IntoBody for Cow<'static, str> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        let content_length = Some(self.len());
        let body = Box::new(Cursor::new(self.as_bytes().to_vec())); // FIXME: don't allocate
        (content_length, body)
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::TEXT_PLAIN_UTF_8)
    }
}

impl IntoBody for &'static [u8] {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        Cow::Borrowed(self).into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::APPLICATION_OCTET_STREAM)
    }
}

impl<const N: usize> IntoBody for &'static [u8; N] {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        self.as_slice().into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::APPLICATION_OCTET_STREAM)
    }
}

impl<const N: usize> IntoBody for [u8; N] {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        self.to_vec().into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::APPLICATION_OCTET_STREAM)
    }
}

impl IntoBody for Vec<u8> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        Cow::<'static, [u8]>::Owned(self).into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::APPLICATION_OCTET_STREAM)
    }
}

impl IntoBody for Box<[u8]> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        Vec::from(self).into_body()
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::APPLICATION_OCTET_STREAM)
    }
}

impl IntoBody for Cow<'static, [u8]> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        let content_length = Some(self.len());
        let body = Box::new(Cursor::new(self.to_vec())); // FIXME: don't allocate
        (content_length, body)
    }

    fn content_type(&self) -> Option<Mime> {
        Some(mime::APPLICATION_OCTET_STREAM)
    }
}

impl<R: Read + 'static> IntoBody for Box<R> {
    fn into_body(self) -> (Option<usize>, Box<dyn Read>) {
        (None, self)
    }

    fn content_type(&self) -> Option<Mime> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles() {
        ().into_body();
        "hello".into_body();
        "hello".to_string().into_body();
        b"hello".into_body();
        [b'h', b'i'].into_body();
        b"hello".to_vec().into_body();
        Box::new(Cursor::new("hello")).into_body();
    }
}
