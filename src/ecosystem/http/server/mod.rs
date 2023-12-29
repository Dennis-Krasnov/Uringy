//! ...

use std::cell::RefCell;
use std::io::{BufWriter, Read, Write};
use std::rc::Rc;

use crate::circular_buffer;
use crate::circular_buffer::circular_buffer;
use crate::ecosystem::http::payload::{Request, Response};
use crate::ecosystem::http::server::route::Router;
use crate::ecosystem::http::{Respond, Responder};
use crate::runtime::{is_cancelled, park, spawn, Waker};

pub mod fake_client;
pub mod route;

/// ...
pub fn serve<W: Write + 'static, R: Read + 'static>(
    router: Router,
    connections: impl Iterator<Item = (W, R)>,
) {
    // TODO: don't need Rc for router when using scoped spawn
    let router = Rc::new(router);

    for (w, r) in connections {
        let router = router.clone();
        // TODO: spawn_contained
        spawn(move || {
            handle_connection(&router, w, r).unwrap();
        });
    }
}

fn handle_connection(
    router: &Router,
    w: impl Write + 'static,
    r: impl Read + 'static,
) -> crate::IoResult<()> {
    // TODO: pool to reuse
    let (mut data, uninit) = circular_buffer(4096)?;
    let waiting_for_data = Rc::new(RefCell::new(None));

    spawn({
        let waiting_for_data = waiting_for_data.clone();
        move || reader(uninit, r, waiting_for_data)
    });

    while !is_cancelled() {
        park(|waker| {
            let mut data = waiting_for_data.borrow_mut();
            *data = Some(waker);
        });

        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut headers);

        match request.parse(&data) {
            Ok(httparse::Status::Complete(wire_size)) => {
                let body_size: usize = request
                    .headers
                    .iter()
                    .find(|h| h.name.to_ascii_lowercase() == "content-length")
                    .and_then(|h| std::str::from_utf8(h.value).ok())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);

                if data.len() < wire_size + body_size {
                    println!("server reading more!");
                    continue;
                }

                let r = Responder(Box::new(RealResponder(Box::new(w))));
                let (path, query) = parse_partial_uri(request.path.unwrap());
                let request = Request {
                    method: request.method.unwrap().parse().unwrap(),
                    path,
                    query,
                    headers: request.headers.iter().map(|h| (h.name, h.value)).collect(),
                    body: &data[wire_size..(wire_size + body_size)],
                };
                router.handle(r, &request);

                data.consume(wire_size);
                println!("exiting");
                break; // FIXME writer should be reusable
            }
            Ok(httparse::Status::Partial) => continue,
            Err(e) => {
                dbg!(e);
                break;
            }
        }
    }

    Ok(())
}

/// Parses path and query out of partial URI (path, query, and fragment).
/// Inspired by https://github.com/hyperium/http/blob/bda93204b3da1a776cf471ed39e8e374cec652e7/src/uri/path.rs#L21-L106.
fn parse_partial_uri(uri: &str) -> (&str, &str) {
    for (i, c) in uri.char_indices() {
        match c as u8 {
            b'?' => return (&uri[..i], parse_query(&uri[i + 1..])),
            b'#' => return (&uri[..i], ""),
            // Code points that don't need to be character encoded
            0x21 | 0x24..=0x3B | 0x3D | 0x40..=0x5F | 0x61..=0x7A | 0x7C | 0x7E => {}
            // JSON should be percent encoded, but still allowed
            b'"' | b'{' | b'}' => {}
            _ => panic!("invalid uri char"), // FIXME Err(InvalidUriChar)
        }
    }

    (uri, "")
}

fn parse_query(query: &str) -> &str {
    for (i, c) in query.char_indices() {
        match c as u8 {
            b'#' => return &query[..i],
            b'?' => {}
            // Should be percent-encoded, but most byes are actually allowed
            0x21 | 0x24..=0x3B | 0x3D | 0x3F..=0x7E => {}
            _ => panic!("invalid uri char"), // FIXME Err(InvalidUriChar)
        }
    }

    query
}

fn reader(
    mut uninit: circular_buffer::Uninit,
    mut r: impl Read,
    waiting_for_data: Rc<RefCell<Option<Waker>>>,
) {
    loop {
        let Ok(bytes_read) = r.read(&mut uninit) else {
            break;
        };

        if bytes_read == 0 {
            break;
        }

        uninit.commit(bytes_read);

        if let Some(waker) = waiting_for_data.borrow_mut().take() {
            waker.schedule();
        }
    }

    // TODO: scoped spawn
    // cancel_propagating();
}

struct RealResponder(Box<dyn Write>);

impl Respond for RealResponder {
    fn respond(self: Box<Self>, response: Response) {
        let mut writer = BufWriter::new(self.0);
        serialize(&mut writer, response).unwrap();
    }
}

fn serialize(mut writer: impl Write, response: Response) -> crate::IoResult<()> {
    writer.write_all(b"HTTP/1.1 ")?;
    let status: u16 = response.status.into();
    writer.write_all(status.to_string().as_bytes())?;
    writer.write_all(b" ")?;
    writer.write_all(b"OK")?;
    // writer.write_all(response.status.canonical_reason().unwrap().as_bytes())?;
    writer.write_all(b"\r\n")?;

    // FIXME: ugly
    writer.write_all("content-length".as_bytes())?;
    writer.write_all(b": ")?;
    writer.write_all(response.body.len().to_string().as_bytes())?;
    writer.write_all(b"\r\n")?;

    writer.write_all(b"connection: close\r\n")?;

    for (name, value) in response.headers {
        writer.write_all(name.as_bytes())?;
        writer.write_all(b": ")?;
        writer.write_all(value)?;
        writer.write_all(b"\r\n")?;
    }
    writer.write_all(b"\r\n")?;
    writer.write_all(response.body)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::ecosystem::http::payload::Method;
    use std::io::{Read, Write};
    use std::net::Ipv4Addr;

    use crate::ecosystem::http::server::route::Router;
    use crate::ecosystem::http::Responder;
    use crate::net::tcp;
    use crate::runtime::{spawn, start};

    use super::*;

    #[test]
    fn end_to_end() {
        start(|| {
            let listener = tcp::Listener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
            let server_addr = listener.local_addr().unwrap();

            let server = spawn(move || {
                let app = Router::new().route(Method::Get, "/", index);
                serve(app, listener.into_incoming());
            });

            let (mut w, mut r) = tcp::connect((Ipv4Addr::LOCALHOST, server_addr.port())).unwrap();

            // TODO: http client
            let request_wire = b"GET / HTTP/1.1\r\ncontent-length: 2\r\n\r\nhi";
            w.write_all(request_wire).unwrap();

            let mut buffer = vec![0; 1024];
            let bytes_read = r.read(&mut buffer).unwrap();
            let response = String::from_utf8_lossy(&buffer[..bytes_read]);
            println!("read:\n{}", response);
            assert!(response.contains("200"));
            // assert_eq!(&buffer[..bytes_read], b"hello");

            server.cancel();
        })
        .unwrap();
    }

    fn index(r: Responder) {
        r.send("hello"); // TODO: include content-length
    }

    #[test]
    fn takes_query_params() {
        start(|| {
            let listener = tcp::Listener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
            let server_addr = listener.local_addr().unwrap();

            let server = spawn(move || {
                let app = Router::new().route(Method::Get, "/", index);
                serve(app, listener.into_incoming());
            });

            let (mut w, mut r) = tcp::connect((Ipv4Addr::LOCALHOST, server_addr.port())).unwrap();

            // TODO: http client
            let request_wire = b"GET /?id=123 HTTP/1.1\r\ncontent-length: 2\r\n\r\nhi";
            w.write_all(request_wire).unwrap();

            let mut buffer = vec![0; 1024];
            let bytes_read = r.read(&mut buffer).unwrap();
            let response = String::from_utf8_lossy(&buffer[..bytes_read]);
            println!("read:\n{}", response);
            assert!(response.contains("200"));

            server.cancel();
        })
        .unwrap();
    }

    mod partial_uri {
        use super::*;

        #[test]
        fn root_path() {
            let (path, query) = parse_partial_uri("/");

            assert_eq!(path, "/");
            assert!(query.is_empty());
        }

        #[test]
        fn path() {
            let (path, query) = parse_partial_uri("/foo/bar");

            assert_eq!(path, "/foo/bar");
            assert!(query.is_empty());
        }

        #[test]
        fn json_path() {
            let (path, query) = parse_partial_uri(r#"/{"foo":"bar"}"#);

            assert_eq!(path, r#"/{"foo":"bar"}"#);
            assert!(query.is_empty());
        }

        #[test]
        fn root_path_with_query() {
            let (path, query) = parse_partial_uri("/?id=123&f");

            assert_eq!(path, "/");
            assert_eq!(query, "id=123&f");
        }

        #[test]
        fn path_with_query() {
            let (path, query) = parse_partial_uri("/foo/bar?id=123&f");

            assert_eq!(path, "/foo/bar");
            assert_eq!(query, "id=123&f");
        }

        #[test]
        fn path_ignores_fragment() {
            let (path, query) = parse_partial_uri("/foo/bar#abc");

            assert_eq!(path, "/foo/bar");
            assert!(query.is_empty());
        }

        #[test]
        fn path_with_query_ignores_fragment() {
            let (path, query) = parse_partial_uri("/foo/bar?id=123&f#abc");

            assert_eq!(path, "/foo/bar");
            assert_eq!(query, "id=123&f");
        }

        #[test]
        fn subsequent_question_marks_are_just_characters() {
            let (path, query) = parse_partial_uri("/?id=123?f");

            assert_eq!(path, "/");
            assert_eq!(query, "id=123?f"); // TODO: make sure serde_urlencoded can parse this
        }

        // TODO
        // #[test]
        // #[should_panic]
        // fn fails_invalid_path() {
        //     parse_partial_uri("/");
        // }
        //
        // #[test]
        // #[should_panic]
        // fn fails_invalid_query() {
        //     parse_partial_uri("/?");
        // }

        #[test]
        fn ignores_valid_percent_encodings() {
            assert_eq!("/a%20b", parse_partial_uri("/a%20b?r=1").0);
            assert_eq!("qr=%31", parse_partial_uri("/a/b?qr=%31").1);
        }

        #[test]
        fn ignores_invalid_percent_encodings() {
            assert_eq!("/a%%b", parse_partial_uri("/a%%b?r=1").0);
            assert_eq!("/aaa%", parse_partial_uri("/aaa%").0);
            assert_eq!("/aaa%", parse_partial_uri("/aaa%?r=1").0);
            assert_eq!("/aa%2", parse_partial_uri("/aa%2").0);
            assert_eq!("/aa%2", parse_partial_uri("/aa%2?r=1").0);
            assert_eq!("qr=%3", parse_partial_uri("/a/b?qr=%3").1);
        }
    }
}
