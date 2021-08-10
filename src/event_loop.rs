#![warn(missing_docs)]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use slotmap::{DefaultKey, SlotMap};
use waker_fn::waker_fn;

/// ...
pub struct EventLoop {
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
    pub fn spawn(&mut self, future: impl Future<Output=()> + 'static) {
        let pinned_future = Box::pin(future);
        let key = self.active_futures.insert(pinned_future);
        self.ready_futures_tx.send(key).unwrap();
    }

    /// ...
    pub fn run(&mut self) {
        while let Ok(key) = self.ready_futures_rx.recv() {
            let future = &mut self.active_futures[key];

            let ready_futures_tx = self.ready_futures_tx.clone();
            let waker = waker_fn(move || {
                ready_futures_tx.send(key).unwrap();
            });

            let mut context = Context::from_waker(&waker);

            match Pin::new(future).poll(&mut context) {
                Poll::Ready(_) => { self.active_futures.remove(key); }
                Poll::Pending => { continue; }
            }

            if self.active_futures.is_empty() {
                break;
            }
        }
    }
}
