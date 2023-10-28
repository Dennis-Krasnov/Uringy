//! ... can't block when cancelled: can read if not empty, can send if not full.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::runtime;
use crate::runtime::is_cancelled;

pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let state = Rc::new(RefCell::new(ChannelState {
        no_longer_empty: VecDeque::new(),
        queue: VecDeque::new(),
        is_closed: false,
    }));

    let tx = Sender(Rc::new(SenderState {
        state: state.clone(),
    }));

    let rx = Receiver(Rc::new(ReceiverState { state }));

    (tx, rx)
}

/// ...
#[derive(Debug, Clone)]
pub struct Sender<T>(Rc<SenderState<T>>);

impl<T> Sender<T> {
    /// ...
    pub fn send(&self, data: T) -> Result<(), crate::Error<ClosedError>> {
        let mut state = self.0.state.borrow_mut();

        if state.is_closed {
            println!("recv: closed");
            return Err(crate::Error::Original(ClosedError));
        }

        state.queue.push_back(data);

        if let Some(waker) = state.no_longer_empty.pop_front() {
            println!("sender send woke {waker:?}");
            waker.schedule();
        }

        Ok(())
    }

    /// ...
    #[inline]
    pub fn len(&self) -> usize {
        let state = self.0.state.borrow();
        state.queue.len()
    }

    /// ...
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// ...
    #[inline]
    pub fn close(&self) {
        self.0.close();
    }

    /// ...
    #[inline]
    pub fn is_closed(&self) -> bool {
        let state = self.0.state.borrow();
        state.is_closed
    }
}

#[derive(Debug)]
struct SenderState<T> {
    state: Rc<RefCell<ChannelState<T>>>,
}

impl<T> SenderState<T> {
    fn close(&self) {
        let mut state = self.state.borrow_mut();
        state.is_closed = true;

        for waker in state.no_longer_empty.drain(..) {
            println!("sender close woke {waker:?}");
            waker.schedule();
        }
    }
}

impl<T> Drop for SenderState<T> {
    fn drop(&mut self) {
        self.close();
    }
}

/// ...
#[derive(Debug, Clone)]
pub struct Receiver<T>(Rc<ReceiverState<T>>);

impl<T> Receiver<T> {
    /// ...
    pub fn recv(&self) -> Result<T, crate::Error<ClosedError>> {
        loop {
            let mut state = self.0.state.borrow_mut();

            if let Some(message) = state.queue.pop_front() {
                println!("recv: value");
                break Ok(message);
            }

            if state.is_closed {
                println!("recv: closed");
                break Err(crate::Error::Original(ClosedError));
            }

            if is_cancelled() {
                println!("recv: cancelled");
                return Err(crate::Error::Cancelled);
            }

            runtime::park(|waker| {
                state.no_longer_empty.push_back(waker);
                drop(state);
            }); // woken up by sender or cancellation
        }
    }

    /// ...
    #[inline]
    pub fn len(&self) -> usize {
        let state = self.0.state.borrow();
        state.queue.len()
    }

    /// ...
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// ...
    #[inline]
    pub fn close(&self) {
        let mut state = self.0.state.borrow_mut();
        state.is_closed = true;
    }

    /// ...
    #[inline]
    pub fn is_closed(&self) -> bool {
        let state = self.0.state.borrow();
        state.is_closed
    }
}

impl<T> Iterator for Receiver<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv().ok()
    }
}

#[derive(Debug)]
struct ReceiverState<T> {
    state: Rc<RefCell<ChannelState<T>>>,
}

impl<T> Drop for ReceiverState<T> {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.is_closed = true;
    }
}

#[derive(Debug)]
struct ChannelState<T> {
    no_longer_empty: VecDeque<runtime::Waker>,
    queue: VecDeque<T>,
    is_closed: bool,
}

/// ...
#[derive(Debug, PartialEq)]
pub struct ClosedError;

#[cfg(test)]
mod tests {
    use runtime::{spawn, start};

    use crate::runtime::cancel;

    use super::*;

    #[test]
    fn send_then_receive() {
        start(|| {
            let (tx, rx) = unbounded();

            tx.send(1).unwrap();
            tx.send(2).unwrap();
            tx.send(3).unwrap();

            assert_eq!(rx.recv(), Ok(1));
            assert_eq!(rx.recv(), Ok(2));
            assert_eq!(rx.recv(), Ok(3));
        })
        .unwrap();
    }

    #[test]
    fn receive_then_send() {
        start(|| {
            let (tx, rx) = unbounded();

            spawn(move || {
                tx.send(1).unwrap();
            });

            assert_eq!(rx.recv(), Ok(1));
            assert_eq!(rx.recv(), Err(crate::Error::Original(ClosedError)));
        })
        .unwrap();
    }

    #[test]
    fn sender_close_stops_recv() {
        start(|| {
            let (tx, rx) = unbounded::<()>();
            let handle = spawn(move || rx.recv());

            tx.close();
            let result = handle.join().unwrap();

            assert_eq!(result, Err(crate::Error::Original(ClosedError)));
        })
        .unwrap();
    }

    #[test]
    fn sender_drop_stops_recv() {
        start(|| {
            let (tx, rx) = unbounded::<()>();
            let handle = spawn(move || rx.recv());

            drop(tx);
            let result = handle.join().unwrap();

            assert_eq!(result, Err(crate::Error::Original(ClosedError)));
        })
        .unwrap();
    }

    #[test]
    fn compatible_with_iterator() {
        start(|| {
            let (tx, rx) = unbounded();
            tx.send(1).unwrap();
            tx.send(2).unwrap();
            tx.send(3).unwrap();
            drop(tx);

            let collected: Vec<_> = rx.into_iter().collect();

            assert_eq!(collected, vec![1, 2, 3]);
        })
        .unwrap();
    }

    mod cancellation {
        use super::*;

        #[test]
        fn can_always_send_to_unbounded_channel() {
            start(|| {
                let (tx, _rx) = unbounded();
                cancel();

                assert!(tx.send(()).is_ok());
            })
            .unwrap();
        }

        #[test]
        fn stops_active_recv() {
            start(|| {
                let (_tx, rx) = unbounded::<()>();
                let handle = spawn(move || rx.recv());

                handle.cancel();
                let result = handle.join().unwrap();

                assert_eq!(result, Err(crate::Error::Cancelled));
            })
            .unwrap();
        }

        #[test]
        fn fails_blocking_recv() {
            start(|| {
                let (tx, rx) = unbounded();
                tx.send(1).unwrap();
                cancel();

                assert_eq!(rx.recv(), Ok(1));
                assert_eq!(rx.recv(), Err(crate::Error::Cancelled));
            })
            .unwrap();
        }
    }
}
