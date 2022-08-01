use crate::{Inner, ManagerState, Message};
use bipbuffer::BipBuffer;
use nats_proto::client_operation::ClientOperation;
use nats_proto::error::NatsProtoError;
use nats_proto::server_operation::ServerOperation;
use std::collections::HashMap;
use std::rc::Rc;
use uringy::sync::channel;

#[derive(Debug)]
pub(crate) struct ReaderState {
    next_sid: u64,
    pub(crate) subscriptions: HashMap<u64, channel::Sender<Message>>,
}

impl ReaderState {
    pub(crate) fn new() -> Self {
        ReaderState {
            next_sid: 0,
            subscriptions: HashMap::new(),
        }
    }

    pub(crate) fn generate_sid(&mut self) -> u64 {
        self.next_sid += 1;
        self.next_sid
    }
}

pub(crate) async fn actor(connection: Rc<Inner>) {
    let mut bipbuffer: BipBuffer<u8> = BipBuffer::new(1024 * 1024);

    let mut local_tcp = None;

    loop {
        // latest tcp and from manager.
        let tcp = loop {
            let mut state = connection.manager_state.borrow_mut();

            match *state {
                ManagerState::Connected { ref mut reader, .. } => {
                    if let Some(tcp) = reader.take() {
                        local_tcp = Some(tcp);
                    }

                    break local_tcp.as_mut().unwrap();
                }
                ManagerState::Disconnected {
                    ref mut connection_established,
                } => {
                    // Give up mutable borrow during await
                    let waiter = connection_established.waiter();
                    drop(state);
                    waiter.await;
                }
            }
        };

        // Write into bipbuffer...
        if let Ok(buffer) = bipbuffer.reserve(1024 * 1024) {
            let bytes_recv = unsafe { tcp.read(buffer) }.await.unwrap_or(0);
            bipbuffer.commit(bytes_recv);

            if bytes_recv == 0 {
                println!("reader detected that server disconnected");
                connection.manager_state.borrow_mut().disconnect();
                continue;
            }
        }

        // Read from bipbuffer...
        while let Some(buffer) = bipbuffer.read() {
            match ServerOperation::decode(buffer) {
                Ok((wire_size, operation)) => {
                    match operation {
                        ServerOperation::Info { .. } => unreachable!(),

                        ServerOperation::Msg {
                            subject,
                            sid,
                            reply_to,
                            payload,
                        } => {
                            // FIXME: borrow held across await point
                            let state = connection.reader_state.borrow_mut();
                            if let Some(sender) = state.subscriptions.get(&sid) {
                                sender
                                    .send(Message {
                                        subject: subject.to_string(),
                                        reply_to: reply_to.map(ToString::to_string),
                                        payload: Vec::from(payload),
                                        headers: HashMap::with_capacity(0),
                                    })
                                    .await;
                            }
                        }

                        ServerOperation::Hmsg { .. } => unreachable!(),

                        ServerOperation::Ok => unreachable!(),

                        ServerOperation::Err { .. } => unreachable!(),

                        ServerOperation::Ping => {
                            connection.write(&ClientOperation::Pong).await;
                        }

                        ServerOperation::Pong => unreachable!(),
                    }

                    bipbuffer.decommit(wire_size);
                }
                Err(NatsProtoError::InvalidProtocol) => panic!("NATS read invalid protocol"),
                Err(NatsProtoError::BufferTooSmall) => break,
            }
        }
    }
}
