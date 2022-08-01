mod manager;
mod reader;
mod subscription;
mod writer;

use crate::manager::ManagerState;
use crate::reader::ReaderState;
use crate::writer::WriterState;
use nats_proto::client_operation::ClientOperation;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io;
use std::rc::Rc;

pub async fn connect(url: &str) -> io::Result<NatsConnection> {
    let nats = NatsConnection::new(url);

    // Wait until initial TCP connection is established
    {
        let mut manager_state = nats.0.manager_state.borrow_mut();
        if let ManagerState::Disconnected {
            ref mut connection_established,
        } = *manager_state
        {
            // Give up mutable borrow during await
            let waiter = connection_established.waiter();
            drop(manager_state);
            waiter.await;
        }
    }

    Ok(nats)
}

// Internally reference counted. User brings their own Rc.
#[derive(Debug)]
pub struct NatsConnection(Rc<Inner>);

impl NatsConnection {
    fn new(url: &str) -> Self {
        // ...
        let connection = Rc::new(Inner {
            writer_state: RefCell::new(WriterState::new(1024 * 1024)),
            reader_state: RefCell::new(ReaderState::new()),
            manager_state: RefCell::new(ManagerState::new()),
        });

        // Spawn background tasks
        uringy::runtime::spawn(manager::actor(connection.clone(), url.to_string()));
        uringy::runtime::spawn(writer::actor(connection.clone()));
        uringy::runtime::spawn(reader::actor(connection.clone()));

        NatsConnection(connection)
    }

    /// Infallible.
    pub async fn publish(&self, subject: &str, payload: impl AsRef<[u8]>) {
        let payload = payload.as_ref();

        self.0
            .write(&ClientOperation::Pub {
                subject,
                reply_to: None,
                payload,
            })
            .await;
    }

    /// ...
    pub async fn subscribe(&self, subject: &str, queue_group: Option<&str>) -> Subscription {
        let subscription = Subscription::new(self.0.clone(), subject, queue_group);
        subscription.subscribe().await;
        subscription
    }

    /// ...
    pub async fn disconnect(self) {
        // TODO: implement as part of async drop

        std::mem::forget(self);
    }
}

impl Drop for NatsConnection {
    fn drop(&mut self) {
        println!("WARN: nats connection dropped without disconnecting");
    }
}

pub use subscription::Subscription;

#[derive(Debug)]
pub struct Message {
    pub subject: String,
    pub reply_to: Option<String>,
    pub payload: Vec<u8>,
    pub headers: HashMap<String, Vec<String>>,
}

#[derive(Debug)]
struct Inner {
    writer_state: RefCell<WriterState>,
    reader_state: RefCell<ReaderState>,
    manager_state: RefCell<ManagerState>,
}

impl Inner {
    pub(crate) async fn write(&self, operation: &ClientOperation<'_>) {
        let estimated_wire_size = operation.estimate_wire_size();

        loop {
            let mut state = self.writer_state.borrow_mut();

            if let Ok(buffer) = state.bipbuffer.reserve(estimated_wire_size) {
                if let Ok(wire_size) = operation.encode(buffer) {
                    state.bipbuffer.commit(wire_size);

                    // Track end boundary of message
                    let previous_boundary = *state.message_boundaries.iter().last().unwrap();
                    state
                        .message_boundaries
                        .push_back(previous_boundary + wire_size);

                    state.no_longer_empty.notify_all();

                    break;
                }
            }

            // Give up mutable borrow during await
            let waiter = state.no_longer_full.waiter();
            drop(state);
            waiter.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod nats_connection {
        use super::*;

        #[test]
        fn implements_traits() {
            use impls::impls;
            use std::fmt::Debug;

            assert!(impls!(NatsConnection: Debug & !Send & !Sync & !Clone));
        }
    }
}
