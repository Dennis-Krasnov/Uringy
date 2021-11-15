//! ...

use async_task::Runnable;
use concurrent_queue::ConcurrentQueue;
use futures_lite::FutureExt;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll};
use waker_fn::waker_fn;

/// ...
#[derive(Debug)]
pub struct EventLoop {
    ready_futures: Arc<ConcurrentQueue<Runnable>>,
}

impl EventLoop {
    /// ...
    pub fn new() -> Self {
        EventLoop {
            ready_futures: Arc::new(ConcurrentQueue::unbounded()),
        }
    }

    /// ...
    pub fn block_on<T: 'static>(&self, future: impl Future<Output = T> + 'static) -> T {
        let task = self.spawn(future); // FIXME: if I make this class' spawn require 'static, this will break!
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
    pub fn run_to_completion<T>(&self, mut task: async_task::Task<T>) -> T {
        let noop_waker = waker_fn(|| {});
        let mut context = Context::from_waker(&noop_waker);

        loop {
            // Poll pending tasks
            while let Ok(runnable) = self.ready_futures.pop() {
                runnable.run();
            }

            // Check if original task is done before blocking
            if let Poll::Ready(output) = task.poll(&mut context) {
                return output;
            }
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
// }
