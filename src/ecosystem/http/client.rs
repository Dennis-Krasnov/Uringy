//! ...

use std::fmt::Debug;
use std::io;
use std::io::{Read, Write};

use http::StatusCode;

use crate::circular_buffer::CircularBuffer;
use crate::ecosystem::http::{Request, Response};

pub fn issue(
    mut connection: impl Read + Write + Debug,
    request: Request,
) -> crate::IoResult<Response> {
    let mut writer = io::BufWriter::new(&mut connection);
    serialize(&mut writer, request)?;
    writer.flush()?;
    let mut connection = writer.into_inner().unwrap();

    deserialize(&mut connection)
}

fn serialize(mut writer: impl Write, request: Request) -> crate::IoResult<()> {
    writer.write_all(request.method().as_str().as_bytes())?;
    writer.write_all(b" ")?;
    writer.write_all(request.uri().to_string().as_bytes())?;
    writer.write_all(b" ")?;
    writer.write_all(format!("{:?}", request.version()).as_bytes())?;
    writer.write_all(b"\r\n")?;

    for (name, value) in request.headers() {
        writer.write_all(name.as_str().as_bytes())?;
        writer.write_all(b": ")?;
        writer.write_all(value.as_bytes())?;
        writer.write_all(b"\r\n")?; // FIXME: still need double \r\n if there's no headers
    }
    writer.write_all(b"\r\n")?;

    let mut body = request.into_body();
    io::copy(&mut body, &mut writer).map_err(crate::Error::from_io_error)?;

    Ok(())
}

fn deserialize(mut reader: impl Read) -> crate::IoResult<Response> {
    let mut buffer = CircularBuffer::new(4096)?;

    loop {
        let bytes_read = reader.read(&mut buffer.uninit())?;
        buffer.commit(bytes_read);

        if bytes_read == 0 {
            panic!("oops"); // TODO: return correct Err
        }

        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut response = httparse::Response::new(&mut headers);

        match response.parse(&buffer.data()) {
            Ok(httparse::Status::Complete(wire_size)) => {
                let mut builder = Response::builder()
                    .version(http::Version::HTTP_11) // TODO: response.version.unwrap()
                    .status(StatusCode::from_u16(response.code.unwrap()).unwrap());

                let body_size: usize = response
                    .headers
                    .iter()
                    .find(|h| h.name.to_ascii_lowercase() == "content-length")
                    .and_then(|h| std::str::from_utf8(h.value).ok())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);

                for httparse::Header { name, value } in response.headers {
                    builder = builder.header(name.to_string(), value.to_vec());
                }

                if buffer.data().len() < wire_size + body_size {
                    println!("client reading more!");
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
