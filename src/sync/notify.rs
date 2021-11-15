use futures_lite::FutureExt;
use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

/// API copied from Tokio ...
/// ... interior mutability ...
#[derive(Debug)]
pub struct Notify(RefCell<inner::Notify>);

impl Notify {
    /// ...
    pub fn new() -> Self {
        Notify(RefCell::new(inner::Notify::new()))
    }

    /// ...
    pub fn notify_one(&self) {
        self.0.borrow_mut().notify_one();
    }

    /// ...
    pub fn notify_all(&self) {
        self.0.borrow_mut().notify_all();
    }

    /// ...
    pub fn notified(&self) -> Waiter {
        Waiter(self.0.borrow_mut().notified())
    }
}

/// Future returned from [`Notify::notified()`]
/// ... interior mutability ...
#[derive(Debug)]
pub struct Waiter(Rc<RefCell<inner::Waiter>>);

impl Future for Waiter {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.borrow_mut().poll(cx)
    }
}

mod inner {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    /// ...
    #[derive(Debug)]
    pub(super) struct Notify {
        waiters: VecDeque<Rc<RefCell<Waiter>>>, // TODO: UnsafeCell
    }

    impl Notify {
        /// ...
        pub(super) fn new() -> Self {
            Notify {
                waiters: VecDeque::with_capacity(1),
            }
        }

        /// ...
        pub(super) fn notify_one(&mut self) {
            if let Some(waiter) = self.waiters.pop_front() {
                waiter.borrow_mut().notify();
            }
        }

        /// ...
        pub(super) fn notify_all(&mut self) {
            for waiter in self.waiters.drain(..) {
                waiter.borrow_mut().notify();
            }
        }

        /// ...
        pub(super) fn notified(&mut self) -> Rc<RefCell<Waiter>> {
            let waiter = Rc::new(RefCell::new(Waiter::new()));
            self.waiters.push_back(waiter.clone());
            waiter
        }
    }

    /// ...
    #[derive(Debug)]
    pub(super) struct Waiter {
        /// ...
        has_been_notified: bool,

        /// ...
        waker: Option<Waker>,
    }

    impl Waiter {
        /// ...
        fn new() -> Self {
            Waiter {
                has_been_notified: false,
                waker: None,
            }
        }

        /// ...
        fn notify(&mut self) {
            self.has_been_notified = true;

            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
        }
    }

    impl Future for Waiter {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.has_been_notified {
                return Poll::Ready(());
            }

            // TODO: optimize https://github.com/tokio-rs/tokio/blob/669bc4476ee7661063709e23f7b38d129ae1e737/tokio/src/sync/notify.rs#L654
            assert!(self.waker.is_none());
            self.waker = Some(cx.waker().clone());

            Poll::Pending
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::timeout::TimeoutExt;
//     use crate::*;
//     use std::rc::Rc;
//     use std::time::Duration;
//
//     // TODO: make tests more elegant
//
//     mod notify_one {
//         use super::*;
//
//         #[test]
//         #[should_panic]
//         fn notify_without_waiter() {
//             block_on(async {
//                 let notify = Notify::new();
//
//                 notify.notify_one();
//
//                 let notified = notify.notified();
//                 notified.timeout(Duration::from_millis(10)).await.unwrap();
//             });
//         }
//
//         #[test]
//         fn notify_with_waiter() {
//             block_on(async {
//                 let notify = Notify::new();
//                 let notified = notify.notified();
//
//                 notify.notify_one();
//
//                 notified.await;
//             });
//         }
//
//         #[test]
//         fn notify_while_waiter_blocked() {
//             block_on(async {
//                 let notify = Rc::new(Notify::new());
//
//                 spawn_notifier(notify.clone());
//
//                 notify.notified().await;
//             });
//
//             fn spawn_notifier(notify: Rc<Notify>) {
//                 spawn(async move {
//                     notify.notify_one();
//                 })
//                 .detach();
//             }
//         }
//
//         #[test]
//         fn wake_one_waiter() {
//             block_on(async {
//                 let notify = Notify::new();
//                 let notified1 = notify.notified();
//                 let notified2 = notify.notified();
//
//                 notify.notify_one();
//
//                 assert!(woken(notified1).await ^ woken(notified2).await);
//             });
//
//             async fn woken(notified: Waiter) -> bool {
//                 notified.timeout(Duration::from_millis(10)).await.is_some()
//             }
//         }
//     }
//
//     mod notify_all {
//         use super::*;
//
//         #[test]
//         #[should_panic]
//         fn notify_without_waiter() {
//             block_on(async {
//                 let notify = Notify::new();
//
//                 notify.notify_all();
//
//                 let notified = notify.notified();
//                 notified.timeout(Duration::from_millis(10)).await.unwrap();
//             });
//         }
//
//         #[test]
//         fn notify_while_waiter_blocked() {
//             block_on(async {
//                 let notify = Rc::new(Notify::new());
//
//                 spawn_notifier(notify.clone());
//
//                 notify.notified().await;
//             });
//
//             fn spawn_notifier(notify: Rc<Notify>) {
//                 spawn(async move {
//                     notify.notify_all();
//                 })
//                 .detach();
//             }
//         }
//
//         #[test]
//         fn wake_all_waiters() {
//             block_on(async {
//                 let notify = Notify::new();
//                 let notified1 = notify.notified();
//                 let notified2 = notify.notified();
//
//                 notify.notify_all();
//
//                 assert!(woken(notified1).await && woken(notified2).await);
//             });
//
//             async fn woken(notified: Waiter) -> bool {
//                 notified.timeout(Duration::from_millis(10)).await.is_some()
//             }
//         }
//     }
// }
