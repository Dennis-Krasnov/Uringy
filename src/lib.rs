#![warn(missing_docs)]

//! ...

use std::future::Future;

use scoped_tls::scoped_thread_local;

use crate::event_loop::EventLoop;

mod event_loop;
pub mod sync;

scoped_thread_local!(static LOCAL_EVENT_LOOP: EventLoop);

/// ...
pub fn block_on<T: 'static>(future: impl Future<Output = T> + 'static) -> T {
    if LOCAL_EVENT_LOOP.is_set() {
        panic!("There's already an uringy::EventLoop running in this thread");
    }

    let event_loop = EventLoop::new();
    LOCAL_EVENT_LOOP.set(&event_loop, || event_loop.block_on(future))
}

/// Spawn an asynchronous task onto this thread's uringy runtime.
pub fn spawn<T: 'static>(future: impl Future<Output = T> + 'static) -> async_task::Task<T> {
    if !LOCAL_EVENT_LOOP.is_set() {
        panic!("There's no uringy::EventLoop running in this thread");
    }

    LOCAL_EVENT_LOOP.with(|event_loop| event_loop.spawn(future))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod block_on {
        use super::*;

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
        #[should_panic]
        fn nested_block_on() {
            block_on(async {
                block_on(async {});
            });
        }
    }

    // mod spawn {
    //     use super::*;
    //     use crate::sync::channel::unbounded::unbounded;
    // 
    //     // FIXME: thread panicked while panicking. aborting.
    //     // TODO: replace async_channel with local_channel
    //     #[test]
    //     fn detached_task() {
    //         block_on(async {
    //             let (s, r) = unbounded();
    // 
    //             spawn(async move {
    //                 s.send(1).unwrap();
    //             })
    //             .detach();
    // 
    //             assert_eq!(r.recv().await, Ok(1));
    //         });
    //     }
    // 
    //     #[test]
    //     fn awaitable_task() {
    //         block_on(async move {
    //             let (s, r) = unbounded();
    // 
    //             s.send(1).unwrap();
    // 
    //             let task = spawn(async move { r.recv().await });
    // 
    //             assert_eq!(task.await, Ok(1));
    //         });
    //     }
    // 
    //     #[test]
    //     #[should_panic]
    //     fn without_runtime() {
    //         spawn(async {}).detach();
    //     }
    // }
}
