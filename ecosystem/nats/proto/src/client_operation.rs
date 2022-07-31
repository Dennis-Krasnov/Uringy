use crate::error::NatsProtoError;

/// A protocol operation sent by the client.
#[derive(Debug, PartialEq)]
pub enum ClientOperation<'a> {
    /// Specify connection information.
    /// `CONNECT <json>\r\n`
    Connect { json: &'a str },

    /// Publish a message to a subject.
    /// `PUB <subject> [reply-to] <#payload_bytes>\r\n[payload]\r\n`
    Pub {
        subject: &'a str,
        reply_to: Option<&'a str>,
        payload: &'a [u8],
    },

    /// Publish a message to a subject, with headers.
    /// `HPUB <subject> [reply-to] <#header_bytes> <#total_bytes>\r\n[headers][payload]\r\n`
    Hpub {
        subject: &'a str,
        reply_to: Option<&'a str>,
        headers: &'a str,
        payload: &'a [u8],
    },

    /// Subscribe to a subject (or subject wildcard).
    /// `SUB <subject> [queue group] <sid>\r\n`
    Sub {
        subject: &'a str,
        queue_group: Option<&'a str>,
        sid: u64,
    },

    /// Unsubscribe (or auto-unsubscribe) from a subject.
    /// `UNSUB <sid> [max_msgs]\r\n`
    Unsub { sid: u64, max_messages: Option<u64> },

    /// Keep alive request.
    /// `PING\r\n`
    Ping,

    /// Keep alive response.
    /// `PONG\r\n`
    Pong,
}

impl<'a> ClientOperation<'a> {
    /// ...
    #[cfg(feature = "client")]
    pub fn encode(&self, buffer: &mut [u8]) -> Result<usize, NatsProtoError> {
        serialization::encode(buffer, self)
    }

    /// ...
    #[cfg(feature = "server")]
    pub fn decode(buffer: &'a [u8]) -> Result<(usize, Self), NatsProtoError> {
        parsing::decode(buffer)
    }

    /// ...
    pub fn estimate_wire_size(&self) -> usize {
        const WHITESPACE: usize = " ".len();
        const NEW_LINE: usize = "\r\n".len();
        const NUMBER: usize = 20; // length of usize::MAX

        match self {
            ClientOperation::Connect { json } => {
                "CONNECT".len() + WHITESPACE + json.len() + NEW_LINE
            }
            ClientOperation::Pub {
                subject,
                reply_to,
                payload,
            } => {
                "PUB".len()
                    + WHITESPACE * 3
                    + NUMBER
                    + subject.len()
                    + reply_to.map(str::len).unwrap_or(0)
                    + payload.len()
                    + NEW_LINE
            }
            ClientOperation::Hpub {
                subject,
                reply_to,
                headers,
                payload,
            } => {
                "HPUB".len()
                    + WHITESPACE * 4
                    + NUMBER * 2
                    + subject.len()
                    + reply_to.map(str::len).unwrap_or(0)
                    + headers.len()
                    + payload.len()
                    + NEW_LINE
            }
            ClientOperation::Sub {
                subject,
                queue_group,
                ..
            } => {
                "SUB".len()
                    + WHITESPACE * 3
                    + NUMBER
                    + subject.len()
                    + queue_group.map(str::len).unwrap_or(0)
                    + NEW_LINE
            }
            ClientOperation::Unsub { .. } => "UNSUB".len() + WHITESPACE * 2 + NUMBER * 2 + NEW_LINE,
            ClientOperation::Ping => "PING".len() + NEW_LINE,
            ClientOperation::Pong => "PONG".len() + NEW_LINE,
        }
    }
}

#[cfg(feature = "client")]
mod serialization {
    use super::*;
    use crate::cursor::Cursor;

    pub(super) fn encode(
        buffer: &mut [u8],
        operation: &ClientOperation,
    ) -> Result<usize, NatsProtoError> {
        let mut cursor = Cursor::new(buffer);
        let mut number_buffer = itoa::Buffer::new();

        match operation {
            &ClientOperation::Connect { json } => {
                cursor.put(b"CONNECT ")?;

                cursor.put(json.as_bytes())?;
            }

            &ClientOperation::Pub {
                subject,
                reply_to,
                payload,
            } => {
                cursor.put(b"PUB ")?;
                cursor.put(subject.as_bytes())?;
                cursor.put(b" ")?;

                if let Some(reply_to) = &reply_to {
                    cursor.put(reply_to.as_bytes())?;
                    cursor.put(b" ")?;
                }

                cursor.put(number_buffer.format(payload.len()).as_bytes())?;
                cursor.put(b"\r\n")?;

                cursor.put(payload)?;
            }

            &ClientOperation::Hpub {
                subject,
                reply_to,
                headers,
                payload,
            } => {
                cursor.put(b"HPUB ")?;
                cursor.put(subject.as_bytes())?;
                cursor.put(b" ")?;

                if let Some(reply_to) = &reply_to {
                    cursor.put(reply_to.as_bytes())?;
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

            &ClientOperation::Sub {
                subject,
                queue_group,
                sid,
            } => {
                cursor.put(b"SUB ")?;
                cursor.put(subject.as_bytes())?;
                cursor.put(b" ")?;

                if let Some(queue_group) = queue_group {
                    cursor.put(queue_group.as_bytes())?;
                    cursor.put(b" ")?;
                }

                cursor.put(number_buffer.format(sid).as_bytes())?;
            }

            &ClientOperation::Unsub { sid, max_messages } => {
                cursor.put(b"UNSUB ")?;
                cursor.put(number_buffer.format(sid).as_bytes())?;

                if let Some(max_messages) = max_messages {
                    cursor.put(b" ")?;
                    cursor.put(number_buffer.format(max_messages).as_bytes())?;
                }
            }

            &ClientOperation::Ping => {
                cursor.put(b"PING")?;
            }

            &ClientOperation::Pong => {
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

        fn encode(operation: &ClientOperation) -> String {
            let mut buffer = [0; 1024];
            let wire_size = operation.encode(&mut buffer).unwrap();
            assert!(operation.estimate_wire_size() >= wire_size);
            str::from_utf8(&buffer[..wire_size]).unwrap().to_string()
        }

        #[test]
        fn connect() {
            let operation = ClientOperation::Connect { json: "123" };
            assert_eq!(encode(&operation), "CONNECT 123\r\n");
        }

        #[test]
        fn publish() {
            let operation = ClientOperation::Pub {
                subject: "foo",
                reply_to: None,
                payload: b"bar",
            };
            assert_eq!(encode(&operation), "PUB foo 3\r\nbar\r\n");
        }

        #[test]
        fn publish_with_reply() {
            let operation = ClientOperation::Pub {
                subject: "foo",
                reply_to: Some("biz"),
                payload: b"bar",
            };
            assert_eq!(encode(&operation), "PUB foo biz 3\r\nbar\r\n");
        }

        #[test]
        fn subscribe() {
            let operation = ClientOperation::Sub {
                subject: "foo",
                queue_group: None,
                sid: 123,
            };
            assert_eq!(encode(&operation), "SUB foo 123\r\n");
        }

        #[test]
        fn subscribe_with_queue_group() {
            let operation = ClientOperation::Sub {
                subject: "foo",
                queue_group: Some("bar"),
                sid: 123,
            };
            assert_eq!(encode(&operation), "SUB foo bar 123\r\n");
        }

        #[test]
        fn unsub() {
            let operation = ClientOperation::Unsub {
                sid: 123,
                max_messages: None,
            };
            assert_eq!(encode(&operation), "UNSUB 123\r\n");
        }

        #[test]
        fn unsub_with_max_msgs() {
            let operation = ClientOperation::Unsub {
                sid: 123,
                max_messages: Some(456),
            };
            assert_eq!(encode(&operation), "UNSUB 123 456\r\n");
        }

        #[test]
        fn ping() {
            let operation = ClientOperation::Ping;
            assert_eq!(encode(&operation), "PING\r\n");
        }

        #[test]
        fn pong() {
            let operation = ClientOperation::Pong;
            assert_eq!(encode(&operation), "PONG\r\n");
        }
    }
}

#[cfg(feature = "server")]
mod parsing {
    use super::*;
    use crate::utils;
    use nom::branch::alt;
    use nom::bytes::streaming::{tag_no_case, take_while1};
    use nom::combinator::{map_res, opt};
    use nom::sequence::{preceded, terminated};
    use simdutf8::basic::from_utf8;

    pub(super) fn decode(buffer: &[u8]) -> Result<(usize, ClientOperation), NatsProtoError> {
        match parse_client_operation(buffer) {
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

    fn parse_client_operation(buffer: &[u8]) -> nom::IResult<&[u8], ClientOperation> {
        terminated(
            alt((
                parse_connect,
                parse_publish,
                parse_header_publish,
                parse_subscribe,
                parse_unsubscribe,
                parse_ping,
                parse_pong,
            )),
            utils::newline,
        )(buffer)
    }

    fn parse_connect(buffer: &[u8]) -> nom::IResult<&[u8], ClientOperation> {
        let (buffer, _) = terminated(tag_no_case("CONNECT"), utils::whitespace)(buffer)?;
        let (buffer, json) = map_res(take_while1(|byte| byte != b'\r'), from_utf8)(buffer)?;
        Ok((buffer, ClientOperation::Connect { json }))
    }

    fn parse_publish(buffer: &[u8]) -> nom::IResult<&[u8], ClientOperation> {
        let (buffer, _) = terminated(tag_no_case("PUB"), utils::whitespace)(buffer)?;
        let (buffer, subject) = terminated(utils::subject, utils::whitespace)(buffer)?;
        let (buffer, reply_to) = opt(terminated(utils::subject, utils::whitespace))(buffer)?;
        let (buffer, payload) = utils::payload(buffer)?;
        Ok((
            buffer,
            ClientOperation::Pub {
                subject,
                reply_to,
                payload,
            },
        ))
    }

    fn parse_header_publish(buffer: &[u8]) -> nom::IResult<&[u8], ClientOperation> {
        let (buffer, _) = terminated(tag_no_case("HPUB"), utils::whitespace)(buffer)?;
        let (buffer, subject) = terminated(utils::subject, utils::whitespace)(buffer)?;
        let (buffer, reply_to) = opt(terminated(utils::subject, utils::whitespace))(buffer)?;
        let (buffer, (headers, payload)) = utils::headers_and_payload(buffer)?;
        Ok((
            buffer,
            ClientOperation::Hpub {
                subject,
                reply_to,
                headers,
                payload,
            },
        ))
    }

    fn parse_subscribe(buffer: &[u8]) -> nom::IResult<&[u8], ClientOperation> {
        let (buffer, _) = terminated(tag_no_case("SUB"), utils::whitespace)(buffer)?;
        let (buffer, subject) = terminated(utils::subject, utils::whitespace)(buffer)?;
        let (buffer, queue_group) = opt(terminated(utils::subject, utils::whitespace))(buffer)?;
        let (buffer, sid) = utils::number(buffer)?;
        Ok((
            buffer,
            ClientOperation::Sub {
                subject,
                queue_group,
                sid,
            },
        ))
    }

    fn parse_unsubscribe(buffer: &[u8]) -> nom::IResult<&[u8], ClientOperation> {
        let (buffer, _) = terminated(tag_no_case("UNSUB"), utils::whitespace)(buffer)?;
        let (buffer, sid) = utils::number(buffer)?;
        let (buffer, max_messages) = opt(preceded(utils::whitespace, utils::number))(buffer)?;
        Ok((buffer, ClientOperation::Unsub { sid, max_messages }))
    }

    fn parse_ping(buffer: &[u8]) -> nom::IResult<&[u8], ClientOperation> {
        let (buffer, _) = tag_no_case("PING")(buffer)?;
        Ok((buffer, ClientOperation::Ping))
    }

    fn parse_pong(buffer: &[u8]) -> nom::IResult<&[u8], ClientOperation> {
        let (buffer, _) = tag_no_case("PONG")(buffer)?;
        Ok((buffer, ClientOperation::Pong))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use core::str;

        fn decode(wire: &str) -> ClientOperation {
            let (wire_size, operation) = ClientOperation::decode(wire.as_bytes()).unwrap();
            assert_eq!(wire.len(), wire_size);
            operation
        }

        #[test]
        fn connect() {
            let operation = ClientOperation::Connect { json: "123" };
            assert_eq!(decode("CONNECT 123\r\n"), operation);
            assert_eq!(decode("connect 123\r\n"), operation);
            assert_eq!(decode("CONNECT\t  123\r\n"), operation);
        }

        #[test]
        fn publish() {
            let operation = ClientOperation::Pub {
                subject: "foo",
                reply_to: None,
                payload: b"bar",
            };
            assert_eq!(decode("PUB foo 3\r\nbar\r\n"), operation);
            assert_eq!(decode("PUB\tfoo   3\r\nbar\r\n"), operation);
            assert_eq!(decode("pub foo 3\r\nbar\r\n"), operation);
        }

        #[test]
        fn publish_with_reply() {
            let operation = ClientOperation::Pub {
                subject: "foo",
                reply_to: Some("biz"),
                payload: b"bar",
            };
            assert_eq!(decode("PUB foo biz 3\r\nbar\r\n"), operation);
            assert_eq!(decode("PUB\tfoo  biz 3\r\nbar\r\n"), operation);
            assert_eq!(decode("pub foo biz 3\r\nbar\r\n"), operation);
        }

        #[test]
        fn subscribe() {
            let operation = ClientOperation::Sub {
                subject: "foo",
                queue_group: None,
                sid: 123,
            };
            assert_eq!(decode("SUB foo 123\r\n"), operation);
            assert_eq!(decode("SUB\tfoo  123\r\n"), operation);
            assert_eq!(decode("sub foo 123\r\n"), operation);
        }

        #[test]
        fn subscribe_with_queue_group() {
            let operation = ClientOperation::Sub {
                subject: "foo",
                queue_group: Some("bar"),
                sid: 123,
            };
            assert_eq!(decode("SUB foo bar 123\r\n"), operation);
            assert_eq!(decode("SUB\tfoo  bar 123\r\n"), operation);
            assert_eq!(decode("sub foo bar 123\r\n"), operation);
        }

        #[test]
        fn unsub() {
            let operation = ClientOperation::Unsub {
                sid: 123,
                max_messages: None,
            };
            assert_eq!(decode("UNSUB 123\r\n"), operation);
            assert_eq!(decode("UNSUB\t  123\r\n"), operation);
            assert_eq!(decode("unsub 123\r\n"), operation);
        }

        #[test]
        fn unsub_with_max_msgs() {
            let operation = ClientOperation::Unsub {
                sid: 123,
                max_messages: Some(456),
            };
            assert_eq!(decode("UNSUB 123 456\r\n"), operation);
            assert_eq!(decode("UNSUB\t123  456\r\n"), operation);
            assert_eq!(decode("unsub 123 456\r\n"), operation);
        }

        #[test]
        fn ping() {
            let operation = ClientOperation::Ping;
            assert_eq!(decode("PING\r\n"), operation);
            assert_eq!(decode("ping\r\n"), operation);
        }

        #[test]
        fn pong() {
            let operation = ClientOperation::Pong;
            assert_eq!(decode("PONG\r\n"), operation);
            assert_eq!(decode("pong\r\n"), operation);
        }
    }
}
