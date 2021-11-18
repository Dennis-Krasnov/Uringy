//! ...

use crate::local_runtime;
use async_task::Runnable;
use concurrent_queue::ConcurrentQueue;
use futures_lite::FutureExt;
use slotmap::SlotMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use waker_fn::waker_fn;

/// ...
#[derive(Debug)]
pub struct EventLoop {
    ready_futures: Arc<ConcurrentQueue<Runnable>>,
    pending_notifies: SlotMap<NotifyKey, Notify>,
}

impl EventLoop {
    /// ...
    pub fn new() -> Self {
        EventLoop {
            ready_futures: Arc::new(ConcurrentQueue::unbounded()),
            pending_notifies: SlotMap::with_key(),
        }
    }

    /// ...
    pub fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        // to not require 'static
        let (runnable, task) = unsafe { async_task::spawn_unchecked(future, self.schedule()) };
        runnable.schedule();

        self.run_to_completion(task)
    }

    /// Spawn an asynchronous task onto the event loop.
    pub fn spawn<T: 'static>(
        &self,
        future: impl Future<Output = T> + 'static,
    ) -> async_task::Task<T> {
        let (runnable, task) = async_task::spawn_local(future, self.schedule());
        runnable.schedule();
        task
    }

    fn schedule(&self) -> impl Fn(Runnable) + Send + Sync + 'static {
        let state = self.ready_futures.clone();

        move |runnable| {
            state.push(runnable).unwrap();
        }
    }

    /// ...
    pub fn generate_notify(&mut self) -> (Notifier, Waiter) {
        let key = self.pending_notifies.insert(Notify::NothingHappened);

        let notifier = Notifier(key);
        let waiter = Waiter(key);

        (notifier, waiter)
    }

    /// ...
    pub fn notify(&mut self, key: NotifyKey) {
        // TODO: return bool whether was already waiting
        let notify = self.pending_notifies.get_mut(key).unwrap();

        if let Notify::Waiting(waker) = notify {
            waker.take().unwrap().wake();
        }

        *notify = Notify::Notified;
    }

    /// ...
    pub fn poll_notify(&mut self, key: NotifyKey, waker: &Waker) -> bool {
        let notify = self.pending_notifies.get_mut(key).unwrap();
        match notify {
            Notify::NothingHappened => {
                *notify = Notify::Waiting(Some(waker.clone()));
                false
            }
            Notify::Notified => true,
            Notify::Waiting(_) => {
                unreachable!();
            }
        }
    }

    /// ...
    pub fn run_to_completion<T>(&self, mut task: async_task::Task<T>) -> T {
        let noop_waker = waker_fn(|| {});
        let mut context = Context::from_waker(&noop_waker);

        loop {
            // Poll pending tasks
            while let Ok(runnable) = self.ready_futures.pop() {
                runnable.run();
            }

            // Check if original task is done before blocking
            // FIXME: this depends on FutureExt
            if let Poll::Ready(output) = task.poll(&mut context) {
                return output;
            }
        }
    }
}

slotmap::new_key_type! {
    /// ...
    pub struct NotifyKey;
}

/// ...
#[derive(Debug)]
enum Notify {
    NothingHappened,
    Notified,
    Waiting(Option<Waker>),
}

/// ...
#[derive(Debug)]
pub struct Notifier(NotifyKey);

impl Notifier {
    /// ...
    pub fn notify(self) {
        local_runtime().expect("...").notify(self.0);
    }
}

/// ...
#[derive(Debug)]
pub struct Waiter(NotifyKey);

impl Future for Waiter {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match local_runtime()
            .expect("...")
            .poll_notify(self.0, cx.waker())
        {
            true => Poll::Ready(()),
            false => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn notify_then_wait() {
        block_on(async {
            let (notifier, waiter) = notify(); // TODO: don't use lib wrapper

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
