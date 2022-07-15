//! ...

use crate::sync::oneshot_notify;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

/// ...
pub fn oneshot_channel<MSG>() -> (Sender<MSG>, Receiver<MSG>) {
    let state = Rc::new(RefCell::new(ChannelState::new()));

    let sender = Sender(state.clone());
    let receiver = Receiver(state);

    (sender, receiver)
}

#[derive(Debug)]
struct ChannelState<MSG> {
    message: Option<MSG>,
    notifier: Option<oneshot_notify::Notifier>,
    waiter: oneshot_notify::Waiter,
    is_closed: bool,
}

impl<MSG> ChannelState<MSG> {
    fn new() -> Self {
        let (notifier, waiter) = oneshot_notify::oneshot_notify();

        ChannelState {
            message: None,
            notifier: Some(notifier),
            waiter,
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
    pub async fn send(self, message: MSG) {
        let mut state = self.0.as_ref().borrow_mut();
        state.message = Some(message);
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
        state.notifier.take().unwrap().notify();
    }
}

/// ...
#[derive(Debug)]
pub struct Receiver<MSG>(Rc<RefCell<ChannelState<MSG>>>);

impl<MSG> Future for Receiver<MSG> {
    type Output = Option<MSG>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.0.as_ref().borrow_mut();

        let waiter = unsafe { Pin::new_unchecked(&mut state.waiter) };
        if waiter.poll(context).is_pending() {
            return Poll::Pending;
        }

        if let Some(message) = state.message.take() {
            return Poll::Ready(Some(message));
        }

        if state.is_closed {
            return Poll::Ready(None);
        }

        Poll::Pending
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
        fn waits_to_receive() {
            runtime::block_on(async {
                // Given
                let (sender, mut receiver) = oneshot_channel();

                // When
                let initially = utils::poll(&mut receiver);
                sender.send(1).await;
                let eventually = utils::poll(&mut receiver);

                // Then
                assert!(initially.is_pending());
                assert!(eventually.is_ready());
            });
        }

        #[test]
        fn receives_nothing_when_channel_closed() {
            runtime::block_on(async {
                // Given
                let (sender, receiver) = oneshot_channel::<()>();

                // When
                sender.close();
                let message = receiver.await;

                // Then
                assert_eq!(message, None);
            });
        }
    }
}
