//! ...

use std::io::Write;
use std::{io, str};

use nom::bytes::streaming as bytes;
use nom::character::streaming as character;
use nom::combinator::*;
use nom::sequence::*;

#[derive(Debug, PartialEq)]
pub(super) enum ClientOperation<'a> {
    /// Sent to server to specify connection information.
    /// Wire protocol: `CONNECT <json>`.
    Connect { json: &'a str },

    /// Publish a message to a subject, with optional reply subject.
    /// Wire protocol: `PUB <subject> [reply-to] <#bytes>\r\n[payload]\r\n`.
    Pub {
        subject: &'a str,
        reply_to: Option<&'a str>,
        payload: &'a [u8],
    },

    /// Publish a message to a subject, with optional reply subject, with headers.
    /// Wire protocol: `HPUB <subject> [reply-to] <#header_bytes> <#total_bytes>\r\n[headers][payload]\r\n`.
    Hpub {
        subject: &'a str,
        reply_to: Option<&'a str>,
        headers: &'a str,
        payload: &'a [u8],
    },

    /// Subscribe to a subject, with optional load balancing.
    /// Wire protocol: `SUB <subject> [queue group] <sid>\r\n`.
    Sub {
        subject: &'a str,
        queue_group: Option<&'a str>,
        sid: u64, // FIXME: sid is alphanumeric
    },

    /// Unsubscribe from subject, optionally after a number of messages.
    /// Wire protocol: `UNSUB <sid> [max_msgs]\r\n`.
    Unsub { sid: u64, max_messages: Option<u64> }, // FIXME: sid is alphanumeric

    /// PING keep-alive message.
    /// Wire protocol: `PING\r\n`.
    Ping,

    /// PONG keep-alive response.
    /// Wire protocol: `PONG\r\n`.
    Pong,
}

impl ClientOperation<'_> {
    pub(super) fn encode(&self, writer: &mut impl Write) -> io::Result<()> {
        let mut numbers = itoa::Buffer::new();

        match *self {
            ClientOperation::Connect { json } => {
                writer.write_all(b"CONNECT ")?;
                writer.write_all(json.as_bytes())?;
            }
            ClientOperation::Pub {
                subject,
                reply_to,
                payload,
            } => {
                writer.write_all(b"PUB ")?;
                writer.write_all(subject.as_bytes())?;
                writer.write_all(b" ")?;
                if let Some(reply_to) = reply_to {
                    writer.write_all(reply_to.as_bytes())?;
                    writer.write_all(b" ")?;
                }
                writer.write_all(numbers.format(payload.len()).as_bytes())?;
                writer.write_all(b"\r\n")?;
                writer.write_all(payload)?;
            }
            ClientOperation::Hpub { .. } => unimplemented!(),
            ClientOperation::Sub {
                subject,
                queue_group,
                sid,
            } => {
                writer.write_all(b"SUB ")?;
                writer.write_all(subject.as_bytes())?;
                writer.write_all(b" ")?;
                if let Some(queue_group) = queue_group {
                    writer.write_all(queue_group.as_bytes())?;
                    writer.write_all(b" ")?;
                }
                writer.write_all(numbers.format(sid).as_bytes())?;
            }
            ClientOperation::Unsub { sid, max_messages } => {
                writer.write_all(b"UNSUB ")?;
                writer.write_all(sid.to_string().as_bytes())?;
                if let Some(max_messages) = max_messages {
                    writer.write_all(b" ")?;
                    writer.write_all(numbers.format(max_messages).as_bytes())?;
                }
            }
            ClientOperation::Ping => writer.write_all(b"PING")?,
            ClientOperation::Pong => writer.write_all(b"PONG")?,
        }

        writer.write_all(b"\r\n")?;

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum ServerOperation<'a> {
    /// Sent to client after initial TCP/IP connection.
    /// Wire protocol: `INFO <json>\r\n`.
    Info { json: &'a str },

    /// Delivers a message payload to a subscriber, with optional reply subject.
    /// Wire protocol: `MSG <subject> <sid> [reply-to] <#bytes>\r\n[payload]\r\n`.
    Msg {
        subject: &'a str,
        sid: u64, // FIXME: sid is alphanumeric
        reply_to: Option<&'a str>,
        payload: &'a [u8],
    },

    /// Delivers a message payload to a subscriber, with optional reply subject, with headers.
    /// Wire protocol: `HMSG <subject> <sid> [reply-to] <#header_bytes> <#total_bytes>\r\n[headers][payload]\r\n`.
    Hmsg {
        subject: &'a str,
        sid: u64, // FIXME: sid is alphanumeric
        reply_to: Option<&'a str>,
        headers: &'a str,
        payload: &'a [u8],
    },

    /// Verbose mode acknowledgment.
    /// Wire protocol: `+OK\r\n`.
    Ok,

    /// Verbose mode protocol error, may cause client disconnection.
    /// Wire protocol: `-ERR <error message>\r\n`.
    Err { message: &'a str },

    /// PING keep-alive message.
    /// Wire protocol: `PING\r\n`.
    Ping,

    /// PONG keep-alive response.
    /// Wire protocol: `PONG\r\n`.
    Pong,
}

impl ServerOperation<'_> {
    pub(super) fn decode(buffer: &[u8]) -> Result<(ServerOperation, usize), NatsProtoError> {
        match parse_server_operation(buffer) {
            Ok((remaining, operation)) => Ok((operation, buffer.len() - remaining.len())),
            Err(err) => match err {
                nom::Err::Incomplete(_) => Err(NatsProtoError::BufferTooSmall),
                nom::Err::Error(_) => Err(NatsProtoError::InvalidProtocol),
                nom::Err::Failure(_) => Err(NatsProtoError::InvalidProtocol),
            },
        }
    }
}

// TODO: this_err
#[derive(Debug)]
pub(super) enum NatsProtoError {
    /// ...
    BufferTooSmall,

    /// ...
    InvalidProtocol,
}

fn parse_server_operation(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
    let (buffer, operation) = nom::branch::alt((
        parse_info, parse_msg, parse_ok, parse_err, parse_ping, parse_pong,
    ))(buffer)?;
    let (buffer, _) = parse_newline(buffer)?;
    Ok((buffer, operation))
}

fn parse_info(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
    let (buffer, _) = bytes::tag_no_case("INFO")(buffer)?;
    let (buffer, _) = parse_whitespace(buffer)?;

    let (buffer, json) = map_res(bytes::take_till1(|byte| byte == b'\r'), str::from_utf8)(buffer)?;

    Ok((buffer, ServerOperation::Info { json }))
}

fn parse_msg(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
    let (buffer, _) = bytes::tag_no_case("MSG")(buffer)?;
    let (buffer, _) = parse_whitespace(buffer)?;

    let (buffer, subject) = parse_subject(buffer)?;
    let (buffer, _) = parse_whitespace(buffer)?;

    let (buffer, sid) = character::u64(buffer)?;
    let (buffer, _) = parse_whitespace(buffer)?;

    let (buffer, reply_to) = opt(terminated(parse_subject, parse_whitespace))(buffer)?;

    let (buffer, payload) = parse_payload(buffer)?;

    Ok((
        buffer,
        ServerOperation::Msg {
            subject,
            sid,
            reply_to,
            payload,
        },
    ))
}

// TODO: parse_hmsg

fn parse_ok(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
    let (buffer, _) = bytes::tag_no_case("+OK")(buffer)?;

    Ok((buffer, ServerOperation::Ok))
}

fn parse_err(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
    let (buffer, _) = bytes::tag_no_case("-ERR ")(buffer)?;

    let (buffer, message) = map_res(bytes::take_till1(|byte| byte == b'\r'), |bytes| {
        str::from_utf8(bytes)
    })(buffer)?;

    Ok((buffer, ServerOperation::Err { message }))
}

fn parse_ping(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
    let (buffer, _) = bytes::tag_no_case("PING")(buffer)?;

    Ok((buffer, ServerOperation::Ping))
}

fn parse_pong(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
    let (buffer, _) = bytes::tag_no_case("PONG")(buffer)?;

    Ok((buffer, ServerOperation::Pong))
}

fn parse_subject(buffer: &[u8]) -> nom::IResult<&[u8], &str> {
    // TODO: while(alphanumeric)|wildcard|fullwildcard delimited with .
    let (buffer, subject) = map_res(
        bytes::take_while1(|byte| {
            nom::character::is_alphabetic(byte) || byte == b'.' || byte == b'*' || byte == b'>'
        }),
        str::from_utf8,
    )(buffer)?;
    Ok((buffer, subject))
}

fn parse_payload(buffer: &[u8]) -> nom::IResult<&[u8], &[u8]> {
    let (buffer, payload_size) = character::u64(buffer)?;
    let (buffer, _) = parse_newline(buffer)?;
    bytes::take(payload_size)(buffer)
}

fn parse_newline(buffer: &[u8]) -> nom::IResult<&[u8], ()> {
    let (buffer, _) = bytes::tag("\r\n")(buffer)?;

    Ok((buffer, ()))
}

fn parse_whitespace(buffer: &[u8]) -> nom::IResult<&[u8], ()> {
    let (buffer, _) = bytes::take_while1(nom::character::is_space)(buffer)?;

    Ok((buffer, ()))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod client_operation {
        use std::io::Cursor;

        use super::*;

        #[test]
        fn encodes_connect() {
            let operation = ClientOperation::Connect { json: "{}" };
            let mut cursor = Cursor::new(vec![]);

            let result = operation.encode(&mut cursor);

            assert!(result.is_ok());
            assert_eq!(cursor.into_inner(), b"CONNECT {}\r\n");
        }

        mod encodes_pub {
            use super::*;

            #[test]
            fn base() {
                let operation = ClientOperation::Pub {
                    subject: "foo",
                    reply_to: None,
                    payload: &[],
                };
                let mut cursor = Cursor::new(vec![]);

                let result = operation.encode(&mut cursor);

                assert!(result.is_ok());
                assert_eq!(cursor.into_inner(), b"PUB foo 0\r\n\r\n");
            }

            #[test]
            fn with_reply() {
                let operation = ClientOperation::Pub {
                    subject: "foo",
                    reply_to: Some("bar"),
                    payload: &[],
                };
                let mut cursor = Cursor::new(vec![]);

                let result = operation.encode(&mut cursor);

                assert!(result.is_ok());
                assert_eq!(cursor.into_inner(), b"PUB foo bar 0\r\n\r\n");
            }

            #[test]
            fn with_payload() {
                let operation = ClientOperation::Pub {
                    subject: "foo",
                    reply_to: None,
                    payload: b"hello",
                };
                let mut cursor = Cursor::new(vec![]);

                let result = operation.encode(&mut cursor);

                assert!(result.is_ok());
                assert_eq!(cursor.into_inner(), b"PUB foo 5\r\nhello\r\n");
            }

            #[test]
            fn with_payload_and_reply() {
                let operation = ClientOperation::Pub {
                    subject: "foo",
                    reply_to: Some("bar"),
                    payload: b"hello",
                };
                let mut cursor = Cursor::new(vec![]);

                let result = operation.encode(&mut cursor);

                assert!(result.is_ok());
                assert_eq!(cursor.into_inner(), b"PUB foo bar 5\r\nhello\r\n");
            }
        }

        mod encodes_sub {
            use super::*;

            #[test]
            fn base() {
                let operation = ClientOperation::Sub {
                    subject: "foo",
                    queue_group: None,
                    sid: 123,
                };
                let mut cursor = Cursor::new(vec![]);

                let result = operation.encode(&mut cursor);

                assert!(result.is_ok());
                assert_eq!(cursor.into_inner(), b"SUB foo 123\r\n");
            }

            #[test]
            fn with_queue_group() {
                let operation = ClientOperation::Sub {
                    subject: "foo",
                    queue_group: Some("biz"),
                    sid: 123,
                };
                let mut cursor = Cursor::new(vec![]);

                let result = operation.encode(&mut cursor);

                assert!(result.is_ok());
                assert_eq!(cursor.into_inner(), b"SUB foo biz 123\r\n");
            }
        }

        mod encodes_unsub {
            use super::*;

            #[test]
            fn base() {
                let operation = ClientOperation::Unsub {
                    sid: 123,
                    max_messages: None,
                };
                let mut cursor = Cursor::new(vec![]);

                let result = operation.encode(&mut cursor);

                assert!(result.is_ok());
                assert_eq!(cursor.into_inner(), b"UNSUB 123\r\n");
            }

            #[test]
            fn with_max_messages() {
                let operation = ClientOperation::Unsub {
                    sid: 123,
                    max_messages: Some(3),
                };
                let mut cursor = Cursor::new(vec![]);

                let result = operation.encode(&mut cursor);

                assert!(result.is_ok());
                assert_eq!(cursor.into_inner(), b"UNSUB 123 3\r\n");
            }
        }

        #[test]
        fn encodes_ping() {
            let operation = ClientOperation::Ping;
            let mut cursor = Cursor::new(vec![]);

            let result = operation.encode(&mut cursor);

            assert!(result.is_ok());
            assert_eq!(cursor.into_inner(), b"PING\r\n");
        }

        #[test]
        fn encodes_pong() {
            let operation = ClientOperation::Pong;
            let mut cursor = Cursor::new(vec![]);

            let result = operation.encode(&mut cursor);

            assert!(result.is_ok());
            assert_eq!(cursor.into_inner(), b"PONG\r\n");
        }
    }

    mod server_operation {
        use super::*;

        #[test]
        fn decodes_info() {
            let wire = b"INFO {}\r\n";

            let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

            assert_eq!(operation, ServerOperation::Info { json: "{}" });
            assert_eq!(wire_size, wire.len());
        }

        mod decodes_msg {
            use super::*;

            #[test]
            fn base() {
                let wire = b"MSG foo 123 0\r\n\r\n";

                let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

                assert_eq!(
                    operation,
                    ServerOperation::Msg {
                        subject: "foo",
                        sid: 123,
                        reply_to: None,
                        payload: &[],
                    }
                );
                assert_eq!(wire_size, wire.len());
            }

            #[test]
            fn with_reply_to() {
                let wire = b"MSG foo 123 bar 0\r\n\r\n";

                let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

                assert_eq!(
                    operation,
                    ServerOperation::Msg {
                        subject: "foo",
                        sid: 123,
                        reply_to: Some("bar"),
                        payload: &[],
                    }
                );
                assert_eq!(wire_size, wire.len());
            }

            #[test]
            fn with_payload() {
                let wire = b"MSG foo 123 5\r\nhello\r\n";

                let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

                assert_eq!(
                    operation,
                    ServerOperation::Msg {
                        subject: "foo",
                        sid: 123,
                        reply_to: None,
                        payload: b"hello",
                    }
                );
                assert_eq!(wire_size, wire.len());
            }

            #[test]
            fn with_payload_and_reply_to() {
                let wire = b"MSG foo 123 bar 5\r\nhello\r\n";

                let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

                assert_eq!(
                    operation,
                    ServerOperation::Msg {
                        subject: "foo",
                        sid: 123,
                        reply_to: Some("bar"),
                        payload: b"hello",
                    }
                );
                assert_eq!(wire_size, wire.len());
            }
        }

        #[test]
        fn decodes_ok() {
            let wire = b"+OK\r\n";

            let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

            assert_eq!(operation, ServerOperation::Ok);
            assert_eq!(wire_size, wire.len());
        }

        #[test]
        fn decodes_error() {
            let wire = b"-ERR oops\r\n";

            let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

            assert_eq!(operation, ServerOperation::Err { message: "oops" });
            assert_eq!(wire_size, wire.len());
        }

        #[test]
        fn decodes_ping() {
            let wire = b"PING\r\n";

            let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

            assert_eq!(operation, ServerOperation::Ping);
            assert_eq!(wire_size, wire.len());
        }

        #[test]
        fn decodes_pong() {
            let wire = b"PONG\r\n";

            let (operation, wire_size) = ServerOperation::decode(wire).unwrap();

            assert_eq!(operation, ServerOperation::Pong);
            assert_eq!(wire_size, wire.len());
        }
    }
}
