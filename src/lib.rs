#![warn(missing_docs)]

use std::cell::{RefCell, UnsafeCell};
use std::future::Future;

use crate::event_loop::EventLoop;

mod event_loop;

thread_local! {
    /// ...
    static CONTEXT: RefCell<Option<UnsafeCell<EventLoop>>> = RefCell::new(None);
}

/// ...
///
/// # Panics
///
/// Panics if nested...
///
/// # Examples
///
/// ### Run on current thread
/// ```
/// uringy::block_on(async {});
/// ```
///
/// ### Run on new thread
/// ```
/// std::thread::spawn(|| {
///     uringy::block_on(async {});
/// }).join().unwrap();
/// ```
pub fn block_on(future: impl Future<Output=()> + 'static) {
    CONTEXT.with(|ctx| {
        assert!(ctx.borrow().is_none());

        *ctx.borrow_mut() = Some(UnsafeCell::new(EventLoop::new()));

        // Obtain multiple mutable references
        let pointer = ctx.borrow().as_ref().unwrap().get();
        let event_loop = unsafe { pointer.as_mut().unwrap() };

        event_loop.spawn(future);
        event_loop.run_to_completion();

        *ctx.borrow_mut() = None;
    });
}

/// ... uses thread local instance of event loop
///
/// This is safe since single threaded... and only run while polling futures
///
/// # Panics
///
/// Panics if called from **outside** of the uringy runtime.
pub fn spawn(future: impl Future<Output=()> + 'static) {
    CONTEXT.with(|ctx| {
        assert!(ctx.borrow().is_some());

        // Obtain multiple mutable references
        let pointer = ctx.borrow().as_ref().unwrap().get();
        let event_loop = unsafe { pointer.as_mut().unwrap() };

        event_loop.spawn(future);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_access_local_event() {
        CONTEXT.with(|local_event_loop| {
            assert!(local_event_loop.borrow().is_none());
        });

        block_on(async {
            CONTEXT.with(|local_event_loop| {
                assert!(local_event_loop.borrow().is_some());
            });
        });

        CONTEXT.with(|local_event_loop| {
            assert!(local_event_loop.borrow().is_none());
        });
    }

    #[test]
    #[should_panic]
    fn nested_block_on() {
        block_on(async {
            block_on(async {});
        });
    }

    #[test]
    fn spawn_success() {
        block_on(async {
            println!("actually running");
            let (s, r) = async_channel::bounded(1);

            spawn(async move {
                s.send(1).await.unwrap();
                s.send(2).await.unwrap();
            });

            assert_eq!(Ok(1), r.recv().await);
            assert_eq!(Ok(2), r.recv().await);
        });
    }

    #[test]
    #[should_panic]
    fn spawn_fail() {
        spawn(async {});
    }

    #[test]
    fn simple() {
        block_on(async {
            println!("HI");
            let (s, r) = async_channel::bounded(1);
            s.send(1).await.unwrap();
            assert_eq!(Ok(1), r.recv().await);
        });
    }
}
