#![warn(missing_docs)]

//! ...

use std::future::Future;

use std::cell::UnsafeCell;

use crate::event_loop::{EventLoop, Notifier, Waiter};

pub mod event_loop;

/// ...
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    if local_runtime().is_some() {
        panic!("Nested block_on is forbidden, consider spawning the task instead.");
    }

    set_local_runtime(Some(EventLoop::new()));

    let result = local_runtime().unwrap().block_on(future);

    set_local_runtime(None);

    result
}

/// Spawn an asynchronous task onto this thread's uringy runtime.
pub fn spawn<T: 'static>(future: impl Future<Output = T> + 'static) -> async_task::Task<T> {
    local_runtime()
        .expect("There's no uringy runtime to spawn the task on, consider blocking on the task instead.")
        .spawn(future)
}

/// ...
pub fn notify() -> (Notifier, Waiter) {
    local_runtime().expect("...").generate_notify()
}

thread_local! {
    /// ...
    pub(crate) static LOCAL_RUNTIME: UnsafeCell<Option<EventLoop>> = UnsafeCell::new(None);
}

/// ...
pub(crate) fn local_runtime() -> Option<&'static mut EventLoop> {
    LOCAL_RUNTIME.with(|runtime| {
        let runtime = unsafe { runtime.get().as_mut().unwrap() };
        runtime.as_mut()
    })
}

/// ...
fn set_local_runtime(event_loop: Option<EventLoop>) {
    // TODO: rename parameter
    LOCAL_RUNTIME.with(|runtime| {
        let runtime = unsafe { runtime.get().as_mut().unwrap() };
        *runtime = event_loop;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    mod block_on {
        use super::*;

        #[test]
        fn consecutive_block_on() {
            block_on(async {});
            block_on(async {});
        }

        #[test]
        fn return_expression() {
            assert_eq!(block_on(async { 1 }), 1);
        }

        #[test]
        fn await_future() {
            block_on(async {
                assert_eq!(async { 1 }.await, 1);
            });
        }

        #[test]
        fn cross_thread_waker() {
            let (s, r) = async_channel::bounded(1);

            std::thread::spawn(move || {
                block_on(async move {
                    s.send(1).await.unwrap();
                });
            });

            block_on(async move {
                assert_eq!(r.recv().await, Ok(1));
            });
        }

        #[test]
        #[should_panic(
            expected = "Nested block_on is forbidden, consider spawning the task instead."
        )]
        fn nested_block_on() {
            block_on(async {
                block_on(async {});
            });
        }
    }

    mod spawn {
        use super::*;
        // use crate::sync::channel::unbounded::unbounded;
        //
        // #[test]
        // fn detached_task() {
        //     block_on(async {
        //         let (s, r) = unbounded();
        //
        //         spawn(async move {
        //             s.send(1).unwrap();
        //         })
        //         .detach();
        //
        //         assert_eq!(r.recv().await, Ok(1));
        //     });
        // }
        //
        // #[test]
        // fn awaitable_task() {
        //     block_on(async move {
        //         let (s, r) = unbounded();
        //
        //         s.send(1).unwrap();
        //
        //         let task = spawn(async move { r.recv().await });
        //
        //         assert_eq!(task.await, Ok(1));
        //     });
        // }

        #[test]
        #[should_panic(
            expected = "There's no uringy runtime to spawn the task on, consider blocking on the task instead."
        )]
        fn without_runtime() {
            spawn(async {}).detach();
        }
    }
}
