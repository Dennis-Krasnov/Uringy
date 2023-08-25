//! ...

use crate::runtime::{context_switch, runtime};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

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
    pub fn send(&self, data: T) -> Option<()> {
        // TODO: Result<(), Cancelled(if bounded)/Closed>
        let mut state = self.0.state.borrow_mut();

        if state.is_closed {
            return None;
        }

        state.queue.push_back(data);

        if let Some(fiber) = state.no_longer_empty.pop_front() {
            unsafe {
                runtime().ready_fibers.push_back(fiber);
            }
        }

        Some(())
    }

    /// ...
    pub fn len(&self) -> usize {
        let state = self.0.state.borrow();
        state.queue.len()
    }

    /// ...
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// ...
    pub fn close(&self) {
        let mut state = self.0.state.borrow_mut();
        state.is_closed = true;
    }

    /// ...
    pub fn is_closed(&self) -> bool {
        let state = self.0.state.borrow();
        state.is_closed
    }
}

#[derive(Debug)]
struct SenderState<T> {
    state: Rc<RefCell<ChannelState<T>>>,
}

impl<T> Drop for SenderState<T> {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        state.is_closed = true;
    }
}

/// ...
#[derive(Debug, Clone)]
pub struct Receiver<T>(Rc<ReceiverState<T>>);

impl<T> Receiver<T> {
    /// ...
    pub fn recv(&self) -> Option<T> {
        // TODO: Result<(), Cancelled/Closed>

        loop {
            let mut state = self.0.state.borrow_mut();

            if let Some(message) = state.queue.pop_front() {
                break Some(message);
            }

            if state.is_closed {
                break None;
            }

            unsafe {
                let running = runtime().running();

                state.no_longer_empty.push_back(running);
                drop(state); // give up mutable borrow during context switch

                let to = runtime().process_io_and_wait();
                let to = runtime().fibers.get(to).continuation;
                let continuation = &mut runtime().fibers.get(running).continuation;
                context_switch::jump(to, continuation); // woken up by sender
            }
        }
    }

    /// ...
    pub fn len(&self) -> usize {
        let state = self.0.state.borrow();
        state.queue.len()
    }

    /// ...
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// ...
    pub fn close(&self) {
        let mut state = self.0.state.borrow_mut();
        state.is_closed = true;
    }

    /// ...
    pub fn is_closed(&self) -> bool {
        let state = self.0.state.borrow();
        state.is_closed
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
    no_longer_empty: VecDeque<crate::runtime::FiberIndex>,
    queue: VecDeque<T>,
    is_closed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime;

    #[test]
    fn send_then_receive() {
        runtime::start(|| {
            let (tx, rx) = unbounded();

            tx.send(1).unwrap();
            tx.send(2).unwrap();
            tx.send(3).unwrap();

            assert_eq!(rx.recv().unwrap(), 1);
            assert_eq!(rx.recv().unwrap(), 2);
            assert_eq!(rx.recv().unwrap(), 3);
        })
    }

    #[test]
    fn receive_then_send() {
        runtime::start(|| {
            let (tx, rx) = unbounded();

            runtime::spawn(move || {
                tx.send(1).unwrap();
            });

            assert_eq!(rx.recv().unwrap(), 1);
        })
    }
}
