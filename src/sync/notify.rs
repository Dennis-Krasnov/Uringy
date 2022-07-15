//! ...

use crate::sync::oneshot_notify;
use std::collections::VecDeque;

/// ...
#[derive(Debug)]
pub struct Notify {
    notifiers: VecDeque<oneshot_notify::Notifier>,
}

impl Notify {
    /// ...
    pub fn new() -> Self {
        Notify {
            notifiers: VecDeque::new(),
        }
    }

    /// ...
    pub fn waiter(&mut self) -> oneshot_notify::Waiter {
        let (notifier, waiter) = oneshot_notify::oneshot_notify();
        self.notifiers.push_back(notifier);
        waiter
    }

    /// ...
    pub fn notify_one(&mut self) {
        if let Some(notifier) = self.notifiers.pop_front() {
            notifier.notify();
        }
    }

    /// ...
    pub fn notify_all(&mut self) {
        for notifier in self.notifiers.drain(..) {
            notifier.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::oneshot_notify;
    use impls::impls;
    use std::fmt::Debug;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    fn poll(waiter: &mut oneshot_notify::Waiter) -> Poll<()> {
        let waker = noop_waker::noop_waker();
        let mut context = Context::from_waker(&waker);
        let waiter = unsafe { Pin::new_unchecked(waiter) };
        waiter.poll(&mut context)
    }

    #[test]
    fn pending_initially() {
        let mut notify = Notify::new();
        let mut waiter = notify.waiter();

        assert!(poll(&mut waiter).is_pending());
    }

    #[test]
    fn pending_if_created_after_notify() {
        let mut notify = Notify::new();

        notify.notify_one();
        notify.notify_all();

        let mut waiter = notify.waiter();
        assert!(poll(&mut waiter).is_pending());
    }

    #[test]
    fn ready_after_notify_one() {
        let mut notify = Notify::new();
        let mut waiter1 = notify.waiter();
        let mut waiter2 = notify.waiter();

        notify.notify_one();

        assert!(poll(&mut waiter1).is_ready());
        assert!(poll(&mut waiter2).is_pending());
    }

    #[test]
    fn ready_after_notify_all() {
        let mut notify = Notify::new();
        let mut waiter1 = notify.waiter();
        let mut waiter2 = notify.waiter();

        notify.notify_all();

        assert!(poll(&mut waiter1).is_ready());
        assert!(poll(&mut waiter2).is_ready());
    }

    #[test]
    fn pending_after_notify_drop() {
        let mut notify = Notify::new();
        let mut waiter = notify.waiter();

        drop(notify);

        assert!(poll(&mut waiter).is_pending());
    }

    #[test]
    fn unaware_of_dropped_waiter() {
        let mut notify = Notify::new();
        let waiter1 = notify.waiter();
        let mut waiter2 = notify.waiter();

        drop(waiter1);
        notify.notify_one();

        assert!(poll(&mut waiter2).is_pending());
    }

    #[test]
    fn trait_implementations() {
        assert!(impls!(Notify: Debug & !Send & !Sync));
    }
}
