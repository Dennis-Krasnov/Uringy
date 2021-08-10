#![warn(missing_docs)]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use slotmap::{DefaultKey, SlotMap};
use waker_fn::waker_fn;

// TODO: more descriptive error messages

/// ...
pub struct EventLoop {
    // TODO: publish to and block on uring (generic)
    ready_futures_tx: crossbeam_channel::Sender<DefaultKey>,
    ready_futures_rx: crossbeam_channel::Receiver<DefaultKey>,
    active_futures: SlotMap<DefaultKey, Pin<Box<dyn Future<Output=()>>>>,
}

impl EventLoop {
    /// ...
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        EventLoop {
            ready_futures_tx: tx,
            ready_futures_rx: rx,
            active_futures: SlotMap::new(),
        }
    }

    /// ...
    pub fn spawn(&mut self, future: impl Future<Output=()> + 'static) { // FIXME: can I get away without 'static?
        let pinned_future = Box::pin(future);
        let key = self.active_futures.insert(pinned_future);
        self.ready_futures_tx.send(key).unwrap();
    }

    /// ...
    pub fn run_to_completion(&mut self) {
        while let Ok(key) = self.ready_futures_rx.recv() {
            let future = &mut self.active_futures[key];

            let ready_futures_tx = self.ready_futures_tx.clone();
            let waker = waker_fn(move || {
                ready_futures_tx.send(key).unwrap();
            });

            let context = &mut Context::from_waker(&waker);

            if Pin::new(future).poll(context).is_ready() {
                self.active_futures.remove(key).unwrap();

                if self.active_futures.is_empty() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple2() {
        let (s, r) = async_channel::unbounded();

        let mut event_loop = EventLoop::new();

        event_loop.spawn(async move {
            for n in 0..10 {
                s.send(n).await.unwrap();
            }
        });

        event_loop.spawn(async move {
            while let Ok(n) = r.recv().await {
                println!("it's {}", n);
            }
        });

        event_loop.run_to_completion();
    }
}
