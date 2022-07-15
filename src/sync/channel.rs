//! Asynchronous channels for message passing.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::rc::Rc;

use crate::sync::notify;

/// ...
pub fn bounded<MSG>(capacity: usize) -> (Sender<MSG>, Receiver<MSG>) {
    if capacity == 0 {
        panic!("...");
    }

    let state = Rc::new(RefCell::new(ChannelState::new(Some(capacity))));

    let sender = Sender(state.clone());
    let receiver = Receiver(state);

    (sender, receiver)
}

/// ...
pub fn unbounded<MSG>() -> (Sender<MSG>, Receiver<MSG>) {
    let state = Rc::new(RefCell::new(ChannelState::new(None)));

    let sender = Sender(state.clone());
    let receiver = Receiver(state);

    (sender, receiver)
}

#[derive(Debug)]
struct ChannelState<MSG> {
    queue: VecDeque<MSG>,
    capacity: Option<usize>,
    no_longer_full: notify::Notify,
    no_longer_empty: notify::Notify, // Used by both Sender::send and Sender::drop
    is_closed: bool,
}

impl<MSG> ChannelState<MSG> {
    fn new(capacity: Option<usize>) -> Self {
        ChannelState {
            queue: VecDeque::with_capacity(capacity.unwrap_or(7)),
            capacity,
            no_longer_full: notify::Notify::new(),
            no_longer_empty: notify::Notify::new(),
            is_closed: false,
        }
    }
}

/// ...
#[derive(Debug)]
pub struct Sender<MSG>(Rc<RefCell<ChannelState<MSG>>>);

impl<MSG> Sender<MSG> {
    /// ...
    /// Infallible.
    pub async fn send(&self, message: MSG) {
        loop {
            let mut state = self.0.as_ref().borrow_mut();

            // Can't use `state.queue.capacity()` since it's infinite if `MSG` is a ZST
            if state.capacity.map_or(true, |cap| state.queue.len() < cap) {
                state.queue.push_back(message);
                state.no_longer_empty.notify_one();
                return;
            }

            // Give up mutable borrow during await
            let waiter = state.no_longer_full.waiter();
            drop(state);
            waiter.await;
        }
    }

    /// ...
    /// close status goes from sender -> receiver.
    /// no info goes from receiver -> sender. do so explicitly.
    pub fn close(self) {
        drop(self);
    }
}

impl<MSG> Drop for Sender<MSG> {
    fn drop(&mut self) {
        let mut state = self.0.as_ref().borrow_mut();

        state.is_closed = true;

        // Will forever remain empty ...
        state.no_longer_empty.notify_all();
    }
}

/// ...
#[derive(Debug)]
pub struct Receiver<MSG>(Rc<RefCell<ChannelState<MSG>>>);

impl<MSG> Receiver<MSG> {
    /// ...
    /// None if sender closed.
    pub async fn recv(&self) -> Option<MSG> {
        loop {
            let mut state = self.0.as_ref().borrow_mut();

            if let Some(message) = state.queue.pop_front() {
                state.no_longer_full.notify_one();
                return Some(message);
            }

            if state.is_closed {
                return None;
            }

            // Give up mutable borrow during await
            let waiter = state.no_longer_empty.waiter();
            drop(state);
            waiter.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime;
    use crate::utils;

    use super::*;

    mod sender {
        use super::*;

        #[test]
        fn implements_traits() {
            use impls::impls;
            use std::fmt::Debug;

            assert!(impls!(Sender<i32>: Debug & !Send & !Sync & !Clone));
        }

        #[test]
        fn conditionally_implements_debug() {
            use impls::impls;
            use std::fmt::Debug;

            // Given
            struct NotDebug;

            // Then
            assert!(impls!(Sender<NotDebug>: !Debug));
        }

        #[test]
        fn is_compatible_with_bounded_and_unbounded() {
            runtime::block_on(async {
                // Given
                let (bounded_sender, _) = bounded::<()>(1);
                let (unbounded_sender, _) = unbounded::<()>();

                // Then
                let _senders = [bounded_sender, unbounded_sender];
            });
        }
    }

    mod receiver {
        use super::*;

        #[test]
        fn implements_traits() {
            use impls::impls;
            use std::fmt::Debug;

            assert!(impls!(Receiver<i32>: Debug & !Send & !Sync & !Clone));
        }

        #[test]
        fn conditionally_implements_debug() {
            use impls::impls;
            use std::fmt::Debug;

            // Given
            struct NotDebug;

            // Then
            assert!(impls!(Receiver<NotDebug>: !Debug));
        }

        #[test]
        fn is_compatible_with_bounded_and_unbounded() {
            runtime::block_on(async {
                // Given
                let (_, bounded_receiver) = bounded::<()>(1);
                let (_, unbounded_receiver) = unbounded::<()>();

                // Then
                let _receivers = [bounded_receiver, unbounded_receiver];
            });
        }
    }

    mod bounded {
        use super::*;

        #[test]
        #[should_panic]
        fn fails_to_create_with_zero_capacity() {
            // When
            bounded::<()>(0);
        }

        mod sender {
            use super::*;

            #[test]
            fn waits_to_send_when_at_capacity() {
                runtime::block_on(async {
                    // Given
                    let (sender, receiver) = bounded(2);
                    sender.send(1).await;
                    sender.send(2).await;
                    let mut send = sender.send(3);

                    // When
                    let initially = utils::poll(&mut send);
                    let _ = receiver.recv().await;
                    let eventually = utils::poll(&mut send);

                    // Then
                    assert!(initially.is_pending());
                    assert!(eventually.is_ready());
                });
            }
        }

        mod receiver {
            use super::*;

            #[test]
            fn waits_to_receive_when_empty() {
                runtime::block_on(async {
                    // Given
                    let (sender, receiver) = bounded(1);
                    let mut recv = receiver.recv();

                    // When
                    let initially = utils::poll(&mut recv);
                    sender.send(1).await;
                    let eventually = utils::poll(&mut recv);

                    // Then
                    assert!(initially.is_pending());
                    assert!(eventually.is_ready());
                });
            }

            #[test]
            fn receives_remaining_messages_when_channel_closed() {
                runtime::block_on(async {
                    // Given
                    let (sender, receiver) = bounded(1);
                    sender.send(1).await;

                    // When
                    sender.close();
                    let messages = [receiver.recv().await, receiver.recv().await];

                    // Then
                    assert_eq!(messages, [Some(1), None]);
                });
            }

            #[test]
            fn receives_remaining_messages_when_channel_dropped() {
                runtime::block_on(async {
                    // Given
                    let (sender, receiver) = bounded(1);
                    sender.send(1).await;

                    // When
                    drop(sender);
                    let messages = [receiver.recv().await, receiver.recv().await];

                    // Then
                    assert_eq!(messages, [Some(1), None]);
                });
            }
        }
    }

    mod unbounded {
        use super::*;

        mod sender {
            use super::*;

            #[test]
            fn never_waits() {
                runtime::block_on(async {
                    // Given
                    let (sender, _) = unbounded();

                    // Then
                    for i in 0..1000 {
                        assert!(utils::poll(&mut sender.send(i)).is_ready());
                    }
                });
            }
        }

        mod receiver {
            use super::*;

            #[test]
            fn waits_to_receive_when_empty() {
                runtime::block_on(async {
                    // Given
                    let (sender, receiver) = unbounded();
                    let mut recv = receiver.recv();

                    // When
                    let initially = utils::poll(&mut recv);
                    sender.send(1).await;
                    let eventually = utils::poll(&mut recv);

                    // Then
                    assert!(initially.is_pending());
                    assert!(eventually.is_ready());
                });
            }

            #[test]
            fn receives_remaining_messages_when_channel_closed() {
                runtime::block_on(async {
                    // Given
                    let (sender, receiver) = unbounded();
                    sender.send(1).await;

                    // When
                    sender.close();
                    let messages = [receiver.recv().await, receiver.recv().await];

                    // Then
                    assert_eq!(messages, [Some(1), None]);
                });
            }

            #[test]
            fn receives_remaining_messages_when_channel_dropped() {
                runtime::block_on(async {
                    // Given
                    let (sender, receiver) = unbounded();
                    sender.send(1).await;

                    // When
                    drop(sender);
                    let messages = [receiver.recv().await, receiver.recv().await];

                    // Then
                    assert_eq!(messages, [Some(1), None]);
                });
            }
        }
    }
}
