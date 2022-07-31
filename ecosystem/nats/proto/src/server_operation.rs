use crate::error::NatsProtoError;

/// A protocol operation sent by the server.
#[derive(Debug, PartialEq)]
pub enum ServerOperation<'a> {
    /// Specify server information.
    /// `INFO <json>`
    Info { json: &'a str },

    /// Deliver a message payload to a subscriber.
    /// `MSG <subject> <sid> [reply-to] <#payload_bytes>\r\n[payload]\r\n`
    Msg {
        subject: &'a str,
        sid: u64,
        reply_to: Option<&'a str>,
        payload: &'a [u8],
    },

    /// Deliver a message payload to a subscriber, with headers.
    /// `HMSG <subject> <sid> [reply-to] <#header_bytes> <#total_bytes>\r\n[headers][payload]\r\n`
    Hmsg {
        subject: &'a str,
        sid: u64,
        reply_to: Option<&'a str>,
        headers: &'a str,
        payload: &'a [u8],
    },

    /// Verbose mode acknowledgment.
    /// `+OK\r\n`
    Ok,

    /// Verbose mode protocol error.
    /// `-ERR <error message>\r\n`
    Err { error_message: &'a str },

    /// Keep alive request.
    /// `PING\r\n`
    Ping,

    /// Keep alive response.
    /// `PONG\r\n`
    Pong,
}

impl<'a> ServerOperation<'a> {
    /// ...
    #[cfg(feature = "server")]
    pub fn encode(&self, buffer: &mut [u8]) -> Result<usize, NatsProtoError> {
        serialization::encode(buffer, self)
    }

    /// ...
    #[cfg(feature = "client")]
    pub fn decode(buffer: &'a [u8]) -> Result<(usize, Self), NatsProtoError> {
        parsing::decode(buffer)
    }

    /// ...
    pub fn estimate_wire_size(&self) -> usize {
        const WHITESPACE: usize = " ".len();
        const NEW_LINE: usize = "\r\n".len();
        const NUMBER: usize = 20; // length of usize::MAX

        match self {
            ServerOperation::Info { json } => "INFO".len() + WHITESPACE + json.len() + NEW_LINE,
            ServerOperation::Msg {
                subject,
                reply_to,
                payload,
                ..
            } => {
                "MSG".len()
                    + WHITESPACE * 3
                    + NUMBER * 2
                    + subject.len()
                    + reply_to.map(str::len).unwrap_or(0)
                    + payload.len()
                    + NEW_LINE
            }
            ServerOperation::Hmsg {
                subject,
                reply_to,
                headers,
                payload,
                ..
            } => {
                "HMSG".len()
                    + WHITESPACE * 4
                    + NUMBER * 3
                    + subject.len()
                    + reply_to.map(str::len).unwrap_or(0)
                    + headers.len()
                    + payload.len()
                    + NEW_LINE
            }
            ServerOperation::Ok => "+OK".len() + NEW_LINE,
            ServerOperation::Err { error_message } => {
                "-ERR".len() + WHITESPACE + error_message.len() + NEW_LINE
            }
            ServerOperation::Ping => "PING".len() + NEW_LINE,
            ServerOperation::Pong => "PONG".len() + NEW_LINE,
        }
    }
}

#[cfg(feature = "server")]
mod serialization {
    use super::*;
    use crate::cursor::Cursor;

    pub(super) fn encode(
        buffer: &mut [u8],
        operation: &ServerOperation,
    ) -> Result<usize, NatsProtoError> {
        let mut cursor = Cursor::new(buffer);
        let mut number_buffer = itoa::Buffer::new();

        match operation {
            &ServerOperation::Info { json } => {
                cursor.put(b"INFO ")?;
                cursor.put(json.as_bytes())?;
            }

            &ServerOperation::Msg {
                subject,
                sid,
                reply_to,
                payload,
            } => {
                cursor.put(b"MSG ")?;
                cursor.put(subject.as_bytes())?;
                cursor.put(b" ")?;

                cursor.put(number_buffer.format(sid).as_bytes())?;
                cursor.put(b" ")?;

                if let Some(reply) = &reply_to {
                    cursor.put(reply.as_bytes())?;
                    cursor.put(b" ")?;
                }

                cursor.put(number_buffer.format(payload.len()).as_bytes())?;
                cursor.put(b"\r\n")?;

                cursor.put(payload)?;
            }

            &ServerOperation::Hmsg {
                subject,
                sid,
                reply_to,
                headers,
                payload,
            } => {
                cursor.put(b"HMSG ")?;
                cursor.put(subject.as_bytes())?;
                cursor.put(b" ")?;

                cursor.put(number_buffer.format(sid).as_bytes())?;
                cursor.put(b" ")?;

                if let Some(reply) = &reply_to {
                    cursor.put(reply.as_bytes())?;
                    cursor.put(b" ")?;
                }

                cursor.put(number_buffer.format(headers.len()).as_bytes())?;
                cursor.put(b" ")?;

                cursor.put(
                    number_buffer
                        .format(headers.len() + payload.len())
                        .as_bytes(),
                )?;
                cursor.put(b"\r\n")?;

                cursor.put(headers.as_bytes())?;
                cursor.put(payload)?;
            }

            &ServerOperation::Ok => {
                cursor.put(b"+OK")?;
            }

            &ServerOperation::Err { error_message } => {
                cursor.put(b"-ERR ")?;
                cursor.put(error_message.as_bytes())?;
            }

            &ServerOperation::Ping => {
                cursor.put(b"PING")?;
            }

            &ServerOperation::Pong => {
                cursor.put(b"PONG")?;
            }
        }

        cursor.put(b"\r\n")?;

        Ok(cursor.position())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use alloc::string::{String, ToString};
        use core::str;

        fn encode(operation: &ServerOperation) -> String {
            let mut buffer = [0; 1024];
            let wire_size = operation.encode(&mut buffer).unwrap();
            assert!(operation.estimate_wire_size() >= wire_size);
            str::from_utf8(&buffer[..wire_size]).unwrap().to_string()
        }

        #[test]
        fn info() {
            let operation = ServerOperation::Info { json: "123" };
            assert_eq!(encode(&operation), "INFO 123\r\n");
        }

        #[test]
        fn msg() {
            let operation = ServerOperation::Msg {
                subject: "foo",
                sid: 123,
                reply_to: None,
                payload: b"bar",
            };
            assert_eq!(encode(&operation), "MSG foo 123 3\r\nbar\r\n");
        }

        #[test]
        fn msg_with_reply() {
            let operation = ServerOperation::Msg {
                subject: "foo",
                sid: 123,
                reply_to: Some("biz"),
                payload: b"bar",
            };
            assert_eq!(encode(&operation), "MSG foo 123 biz 3\r\nbar\r\n");
        }

        #[test]
        fn ok() {
            let operation = ServerOperation::Ok;
            assert_eq!(encode(&operation), "+OK\r\n");
        }

        #[test]
        fn err() {
            let operation = ServerOperation::Err {
                error_message: "'ah shit'",
            };
            assert_eq!(encode(&operation), "-ERR 'ah shit'\r\n");
        }

        #[test]
        fn ping() {
            let operation = ServerOperation::Ping;
            assert_eq!(encode(&operation), "PING\r\n");
        }

        #[test]
        fn pong() {
            let operation = ServerOperation::Pong;
            assert_eq!(encode(&operation), "PONG\r\n");
        }
    }
}

#[cfg(feature = "client")]
mod parsing {
    use super::*;
    use crate::utils;
    use nom::branch::alt;
    use nom::bytes::streaming::{tag_no_case, take_till1};
    use nom::combinator::{map_res, opt};
    use nom::sequence::terminated;
    use simdutf8::basic::from_utf8;

    pub(super) fn decode(buffer: &[u8]) -> Result<(usize, ServerOperation), NatsProtoError> {
        match parse_server_operation(buffer) {
            Ok((remaining, client_operation)) => {
                Ok((buffer.len() - remaining.len(), client_operation))
            }
            Err(err) => match err {
                nom::Err::Incomplete(_) => Err(NatsProtoError::BufferTooSmall),
                nom::Err::Error(_) => Err(NatsProtoError::InvalidProtocol),
                nom::Err::Failure(_) => Err(NatsProtoError::InvalidProtocol),
            },
        }
    }

    fn parse_server_operation(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
        terminated(
            alt((
                parse_info,
                parse_message,
                parse_header_message,
                parse_ok,
                parse_err,
                parse_ping,
                parse_pong,
            )),
            utils::newline,
        )(buffer)
    }

    fn parse_info(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
        let (buffer, _) = terminated(tag_no_case("INFO"), utils::whitespace)(buffer)?;
        let (buffer, json) = map_res(take_till1(|byte| byte == b'\r'), from_utf8)(buffer)?;
        Ok((buffer, ServerOperation::Info { json }))
    }

    fn parse_message(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
        let (buffer, _) = terminated(tag_no_case("MSG"), utils::whitespace)(buffer)?;
        let (buffer, subject) = terminated(utils::subject, utils::whitespace)(buffer)?;
        let (buffer, sid) = terminated(utils::number, utils::whitespace)(buffer)?;
        let (buffer, reply_to) = opt(terminated(utils::subject, utils::whitespace))(buffer)?;
        let (buffer, payload) = utils::payload(buffer)?;
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

    fn parse_header_message(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
        let (buffer, _) = terminated(tag_no_case("HMSG"), utils::whitespace)(buffer)?;
        let (buffer, subject) = terminated(utils::subject, utils::whitespace)(buffer)?;
        let (buffer, sid) = terminated(utils::number, utils::whitespace)(buffer)?;
        let (buffer, reply_to) = opt(terminated(utils::subject, utils::whitespace))(buffer)?;
        let (buffer, (headers, payload)) = utils::headers_and_payload(buffer)?;
        Ok((
            buffer,
            ServerOperation::Hmsg {
                subject,
                sid,
                reply_to,
                headers,
                payload,
            },
        ))
    }

    fn parse_ok(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
        let (buffer, _) = tag_no_case("+OK")(buffer)?;
        Ok((buffer, ServerOperation::Ok))
    }

    fn parse_err(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
        let (buffer, _) = terminated(tag_no_case("-ERR"), utils::whitespace)(buffer)?;
        let (buffer, error_message) = map_res(take_till1(|byte| byte == b'\r'), from_utf8)(buffer)?;
        Ok((buffer, ServerOperation::Err { error_message }))
    }

    fn parse_ping(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
        let (buffer, _) = tag_no_case("PING")(buffer)?;
        Ok((buffer, ServerOperation::Ping))
    }

    fn parse_pong(buffer: &[u8]) -> nom::IResult<&[u8], ServerOperation> {
        let (buffer, _) = tag_no_case("PONG")(buffer)?;
        Ok((buffer, ServerOperation::Pong))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn decode(wire: &str) -> ServerOperation {
            let (wire_size, operation) = ServerOperation::decode(wire.as_bytes()).unwrap();
            assert_eq!(wire.len(), wire_size);
            operation
        }

        #[test]
        fn info() {
            let operation = ServerOperation::Info { json: "123" };
            assert_eq!(decode("INFO 123\r\n"), operation);
            assert_eq!(decode("info 123\r\n"), operation);
            assert_eq!(decode("INFO\t  123\r\n"), operation);
        }

        #[test]
        fn msg() {
            let operation = ServerOperation::Msg {
                subject: "foo",
                sid: 123,
                reply_to: None,
                payload: b"bar",
            };
            assert_eq!(decode("MSG foo 123 3\r\nbar\r\n"), operation);
            assert_eq!(decode("msg foo 123 3\r\nbar\r\n"), operation);
            assert_eq!(decode("MSG\tfoo  123  3\r\nbar\r\n"), operation);
        }

        #[test]
        fn msg_with_reply() {
            let operation = ServerOperation::Msg {
                subject: "foo",
                sid: 123,
                reply_to: Some("_$Z.abc.123"),
                payload: b"bar",
            };
            assert_eq!(decode("MSG foo 123 _$Z.abc.123 3\r\nbar\r\n"), operation);
            assert_eq!(decode("msg foo 123 _$Z.abc.123 3\r\nbar\r\n"), operation);
            assert_eq!(decode("MSG\tfoo  123 _$Z.abc.123  3\r\nbar\r\n"), operation);
        }

        #[test]
        fn ok() {
            let operation = ServerOperation::Ok;
            assert_eq!(decode("+OK\r\n"), operation);
            assert_eq!(decode("+ok\r\n"), operation);
        }

        #[test]
        fn err() {
            let operation = ServerOperation::Err {
                error_message: "'ah shit'",
            };
            assert_eq!(decode("-ERR 'ah shit'\r\n"), operation);
            assert_eq!(decode("-err 'ah shit'\r\n"), operation);
            assert_eq!(decode("-ERR\t  'ah shit'\r\n"), operation);
        }

        #[test]
        fn ping() {
            let operation = ServerOperation::Ping;
            assert_eq!(decode("PING\r\n"), operation);
            assert_eq!(decode("ping\r\n"), operation);
        }

        #[test]
        fn pong() {
            let operation = ServerOperation::Pong;
            assert_eq!(decode("PONG\r\n"), operation);
            assert_eq!(decode("pong\r\n"), operation);
        }
    }
}
