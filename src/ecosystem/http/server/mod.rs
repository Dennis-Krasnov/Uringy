//! ...

use crate::circular_buffer;
use crate::circular_buffer::circular_buffer;
use crate::ecosystem::http::payload::{Request, Response};
use crate::ecosystem::http::server::route::Router;
use crate::ecosystem::http::{Respond, Responder};
use crate::runtime::{cancel_propagating, is_cancelled, park, spawn, Waker};
use std::cell::RefCell;
use std::io::{BufWriter, Read, Write};
use std::rc::Rc;

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
    mut w: impl Write + 'static,
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
                let request = Request {
                    method: request.method.unwrap().parse().unwrap(),
                    path: request.path.unwrap(),
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
    use std::io::{Read, Write};
    use std::net::Ipv4Addr;

    use crate::ecosystem::http::server::route::{get, Router};
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
                let app = Router::new().route("/", get(index));
                serve(app, listener.into_incoming());
            });

            let (mut w, mut r) = tcp::connect((Ipv4Addr::LOCALHOST, server_addr.port())).unwrap();

            // TODO: http client
            let request_wire = b"GET / HTTP/1.1\r\ncontent-length: 2\r\n\r\nhi";
            w.write_all(request_wire).unwrap();

            let mut buffer = vec![0; 1024];
            let bytes_read = r.read(&mut buffer).unwrap();
            println!("read:\n{}", String::from_utf8_lossy(&buffer[..bytes_read]));
            // assert_eq!(&buffer[..bytes_read], b"hello");

            server.cancel();
        })
        .unwrap();
    }

    fn index(r: Responder) {
        r.send("hello"); // TODO: include content-length
    }
}
