use crate::{Inner, Message};
use nats_proto::client_operation::ClientOperation;
use std::rc::Rc;
use uringy::sync::channel;

/// ...
pub struct Subscription {
    connection: Rc<Inner>,
    subject: String,
    queue_group: Option<String>,
    sid: u64, // doesn't change across reconnects
    inbox: channel::Receiver<Message>,
}

impl Subscription {
    /// ...
    pub async fn next(&self) -> Message {
        self.inbox.recv().await.unwrap()
    }

    /// ...
    pub async fn unsubscribe(self, max_messages: Option<u64>) {
        // TODO: implement as part of async drop

        self.connection
            .write(&ClientOperation::Unsub {
                sid: self.sid,
                max_messages,
            })
            .await;

        std::mem::forget(self);
    }

    pub(crate) fn new(connection: Rc<Inner>, subject: &str, queue_group: Option<&str>) -> Self {
        let (sid, inbox) = {
            let mut state = connection.reader_state.borrow_mut();

            // ...
            let sid = state.generate_sid();
            let (s, r) = channel::bounded(1024);
            state.subscriptions.insert(sid, s);

            (sid, r)
        };

        Subscription {
            connection,
            subject: subject.to_string(),
            queue_group: queue_group.map(ToString::to_string),
            sid,
            inbox,
        }
    }

    pub(crate) async fn subscribe(&self) {
        self.connection
            .write(&ClientOperation::Sub {
                subject: &self.subject,
                queue_group: self.queue_group.as_ref().map(|s| s.as_str()),
                sid: self.sid,
            })
            .await;
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        println!("WARN: nats subscription dropped without unsubscribing");
    }
}
