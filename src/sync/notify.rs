//! ...

use std::cell::UnsafeCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

/// ...
pub fn notify() -> (Notifier, Waiter) {
    let notify = Rc::new(UnsafeCell::new(Notify::new()));

    let notifier = Notifier::new(notify.clone());
    let waiter = Waiter::new(notify);

    (notifier, waiter)
}

/// ...
pub struct Notifier {
    notify: Rc<UnsafeCell<Notify>>,
}

impl Notifier {
    /// ...
    fn new(notify: Rc<UnsafeCell<Notify>>) -> Self {
        Notifier { notify }
    }

    /// ...
    pub fn notify(self) {
        let mut notify = unsafe { self.notify.get().as_mut().unwrap() };

        notify.has_been_notified = true;

        if let Some(waker) = notify.waker.take() {
            waker.wake();
        }
    }
}

/// ...
pub struct Waiter {
    notify: Rc<UnsafeCell<Notify>>,
}

impl Waiter {
    /// ...
    fn new(notify: Rc<UnsafeCell<Notify>>) -> Self {
        Waiter { notify }
    }
}

impl Future for Waiter {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut notify = unsafe { self.notify.get().as_mut().unwrap() };

        if notify.has_been_notified {
            return Poll::Ready(());
        }

        assert!(notify.waker.is_none());
        notify.waker = Some(cx.waker().clone());

        Poll::Pending
    }
}

/// ...
struct Notify {
    /// ...
    has_been_notified: bool,

    /// ...
    waker: Option<Waker>,
}

impl Notify {
    /// ...
    fn new() -> Self {
        Notify {
            has_been_notified: false,
            waker: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn temp() {
        
    }
    
    #[test]
    fn notify_then_wait() {
        block_on(async {
            let (notifier, waiter) = notify();

            notifier.notify();

            waiter.await;
        });
    }

    #[test]
    fn wait_then_notify_other_task() {
        block_on(async {
            let (notifier, waiter) = notify();

            spawn(async move {
                notifier.notify();
            })
            .detach();

            waiter.await;
        });
    }

    // #[test]
    // #[should_panic]
    // fn wait_then_notify_deadlock() {
    //     block_on(async {
    //         let (notifier, waiter) = notify();
    //         waiter.await; // TODO: add a timeout, unwrap it
    //         notifier.notify();
    //     });
    // }
}
