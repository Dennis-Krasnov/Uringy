//! Async Rust interface for Linux's io_uring.

use crate::runtime::task;
use async_channel::Sender;
use slotmap::Key;
use slotmap::{DefaultKey, KeyData, SlotMap};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

thread_local! {
    /// When a task is woken, it schedules the task onto the runtime on the current thread.
    /// This may happen after the runtime finishes, so the runtime must be globally accessible and optional.
    /// Thread local storage is ideal since for a given thread, a single runtime can exist at a time.
    ///
    /// The design of the event loop is inseparable from thread local storage.
    /// So for better cohesion the thread local storage is kept private in this module.
    /// Other uringy modules use [`spawn`] and [`syscall`] to interact with the runtime.
    static LOCAL_RUNTIME: RefCell<Option<EventLoop>> = RefCell::new(None);
}

/// Used to generate unique identifiers among Uringy runtimes in this process.
static ID_GENERATOR: AtomicUsize = AtomicUsize::new(0);

/// An instance of the Uringy runtime, created and destroyed by [`block_on`].
/// Carries the state that's used across [`block_on`], [`schedule`], [`syscall`] functions.
///
/// In async Rust terms, the EventLoop acts as both the Reactor and Executor.
struct EventLoop {
    /// Low-level io_uring instance.
    io_uring: io_uring::IoUring,

    /// Links in-flight syscalls to their awaiting tasks.
    syscall_results: RefCell<SlotMap<DefaultKey, Sender<io::Result<u32>>>>,

    /// The last time the submission queue was submitted to the kernel.
    ///
    /// Used to batch submission queue entries.
    last_submit: RefCell<Instant>, // TODO: can be combined with syscall_results' RefCell

    /// Queue of tasks that have been scheduled and are ready to be run.
    ///
    /// Unlike Tokio, this isn't a concurrent queue, scheduling a task from another thread requires IPC.
    ready_tasks: RefCell<VecDeque<task::TaskHandle>>,

    /// Unique identifier among Uringy runtimes in this process.
    ///
    /// Used to ensure Uringy resources are used on the same runtime they were created on.
    /// Used to determine whether task scheduling may be done directly (from the same runtime) or requires IPC.
    _runtime_id: usize,
}

impl EventLoop {
    /// Creates a new instance of the Uringy runtime.
    fn new(config: &Config) -> std::io::Result<Self> {
        Ok(EventLoop {
            io_uring: io_uring::IoUring::new(config.sq_size)?, // TODO: new_with_flags, on separate line (above return)
            syscall_results: RefCell::new(SlotMap::new()),
            last_submit: RefCell::new(Instant::now()),
            ready_tasks: RefCell::new(VecDeque::with_capacity(1024)),
            _runtime_id: ID_GENERATOR.fetch_add(1, Ordering::SeqCst),
        })
    }

    /// Polls the [`future`] until completion, while multitasking background tasks.
    ///
    /// This is a separate function from [`block_on`] since it doesn't concern itself with thread local state. // TODO: same with other global functions
    fn run_to_completion<OUT>(&self, future: impl Future<Output = OUT>, config: &Config) -> OUT {
        // Avoid borrow_mut living for the duration of the while loop's body
        // If this is inlined, a call to [`schedule`] during [`task.run()`] would cause a double mutable borrow
        let next_ready_task = || self.ready_tasks.borrow_mut().pop_front();

        // Boilerplate for polling the original task's output
        let mut future_output = spawn(future);

        let noop_waker = waker_fn::waker_fn(|| {});
        let mut context = Context::from_waker(&noop_waker);

        let mut poll_future_output = || {
            // Pin to stack // TODO: safety explanation
            let future_output = unsafe { Pin::new_unchecked(&mut future_output) };
            future_output.poll(&mut context)
        };

        // For every completed syscall, sends its result to the awaiting task.
        let process_completion_queue = || {
            // Safety: No other completion queue exists
            for cqe in unsafe { self.io_uring.completion_shared() } {
                let result = if cqe.result() >= 0 {
                    Ok(cqe.result() as u32)
                } else {
                    std::io::Result::Err(std::io::Error::from_raw_os_error(-cqe.result()))
                };

                let key = DefaultKey::from(KeyData::from_ffi(cqe.user_data()));
                let sender = self.syscall_results.borrow_mut().remove(key).unwrap(); // TODO: expect
                sender.try_send(result).unwrap(); // TODO: expect // triggers schedule, which mutates ready tasks
            }
        };

        loop {
            while let Some(task) = next_ready_task() {
                // Keep track of how long it takes to poll the task
                let before = Instant::now();

                task.run();

                let _poll_duration = before.elapsed();
                // println!("task poll took {poll_duration:?}");

                // Safety: No other submission queue exists
                let sqe_exists = !unsafe { self.io_uring.submission_shared() }.is_empty();
                let waited_too_long = self.last_submit.borrow().elapsed() > config.sqe_max_linger;
                if sqe_exists && waited_too_long {
                    self.io_uring.submit().expect("...");
                    *self.last_submit.borrow_mut() = Instant::now();
                }

                process_completion_queue();
            }

            if let Poll::Ready(output) = poll_future_output() {
                return output.expect("...");
            }

            // Block the thread until a syscall completes
            self.io_uring.submit_and_wait(1).expect("...");
            *self.last_submit.borrow_mut() = Instant::now();

            process_completion_queue();
        }
    }
}

// impl Debug for EventLoop {
//     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
//         // custom impl since io_uring doesn't implement debug
//         todo!()
//     }
// }

/// ...
/// Blocks the current thread on a future, processing I/O events when idle. ???
/// When the original future completes, the other tasks are cancelled.
///
/// block_on(connect()) === let fut = connect(); block_on(fut); which obviously fails. fixed with async block/function
pub fn block_on<OUT>(future: impl Future<Output = OUT>, config: &Config) -> OUT {
    LOCAL_RUNTIME.with(|local_runtime| {
        // Immutable borrow because block_on may be attempted to run within another block_on.
        // If this where to borrow mutably, would be an issue ???
        if local_runtime.borrow().is_some() {
            // TODO: make error message shorter. make consistent with others.
            panic!(
                "Nested block_on is forbidden, consider spawning a task for the future instead."
            );
        }

        // TODO: pass config values to event loop? put config into event loop?
        let event_loop = EventLoop::new(config).expect("AAAAAAAAAAAAA");
        *local_runtime.borrow_mut() = Some(event_loop);

        let output = local_runtime
            .borrow()
            .as_ref()
            .unwrap()
            .run_to_completion(future, config);

        // TODO: document order of operations
        // only necessary because async_channel wakes channels on drop.
        // destructors running when running `*local_runtime.borrow_mut() = None;` see local_runtime as already mutably borrowed. must do in two steps.
        let event_loop = local_runtime.borrow_mut().take().unwrap();
        drop(event_loop);

        output
    })
}

/// Spawn an asynchronous task onto the event loop.
pub fn spawn<OUT>(future: impl Future<Output = OUT>) -> task::JoinHandle<OUT> {
    // ...
    fn schedule(task: task::TaskHandle) {
        // TODO: schedule should also take target_runtime_id as a parameter.
        LOCAL_RUNTIME.with(|local_runtime| {
            match local_runtime.borrow().as_ref() {
                Some(event_loop) => {
                    // TODO: if event_loop.id == target_runtime_id { ... } else { ... }
                    // assert_eq!(event_loop.thread_id, thread::current().id()); // TODO: should still work
                    // if event_loop.runtime_id != thread::current().id() {
                    //     println!("SCHEDULE THREAD ID DOESN'T MATCH");
                    //     std::process::abort();
                    // }

                    event_loop.ready_tasks.borrow_mut().push_back(task);
                }
                None => {
                    // TODO: should still work (send to the runtime using IPC)
                    println!("WARNING: SCHEDULE CALLED BUT EVENT LOOP DOESN'T EXIST!!!");
                }
            }
        });
    }

    LOCAL_RUNTIME.with(|local_runtime| {
        if local_runtime.borrow().is_none() {
            panic!("There's no uringy runtime to spawn the task on, consider blocking on the future instead.");
        }

        task::create(future, schedule)
    })
}

/// ...
pub(crate) async fn syscall(entry: io_uring::squeue::Entry) -> std::io::Result<u32> {
    // Use channel to ... wait for the result of the syscall
    let (s, r) = async_channel::bounded(1);

    LOCAL_RUNTIME.with(|local_runtime| {
        // TODO: defensive, expect with error message?
        match local_runtime.borrow().as_ref() {
            Some(event_loop) => {
                let key = event_loop.syscall_results.borrow_mut().insert(s);

                // Safety: No other submission queue exists
                let mut sq = unsafe { event_loop.io_uring.submission_shared() };

                // Otherwise the sqe might not be batched (if it's been a while since the last submission)
                if sq.is_empty() {
                    *event_loop.last_submit.borrow_mut() = Instant::now();
                }

                // Otherwise the submission queue would overflow
                if sq.is_full() {
                    event_loop.io_uring.submit().expect("...");
                    *event_loop.last_submit.borrow_mut() = Instant::now();
                }

                // Safety: TODO: explain
                unsafe {
                    sq.push(&entry.user_data(key.data().as_ffi())).unwrap();
                }
            }
            None => panic!("WARNING: TCP SEND CALLED BUT EVENT LOOP DOESN'T EXIST!!!"),
        }
    });

    r.recv().await.expect("...")
}

/// ...
/// Document that you can do:
/// Config {
///   // your stuff here,
///   ..Config::default()
/// }
#[derive(Debug, Clone)] // FIXME: do I need clone?
pub struct Config {
    /// According to iou: The number of entries must be in the range of 1..4096 (inclusive) and it's recommended to be a power of two.
    /// Submission queue.
    sq_size: u32,

    /// ...
    /// System calls.
    _max_inflight_syscalls: u32,

    /// ...
    /// Submission queue entry.
    sqe_max_linger: Duration, // TODO: rename to max_sqe_linger: Duration
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sq_size: 4096,
            _max_inflight_syscalls: 4096, // TODO: pick and document sensible default. (a multiple bigger than sq_size)
            sqe_max_linger: Duration::from_millis(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    #[test]
    fn consecutive() {
        block_on(async {}, &Config::default());
        block_on(async {}, &Config::default());
    }

    #[test]
    #[should_panic]
    fn nested() {
        block_on(
            async {
                block_on(async {}, &Config::default());
            },
            &Config::default(),
        );
    }

    #[test]
    fn return_output() {
        let result = block_on(async { 123 }, &Config::default());

        assert_eq!(result, 123);
    }

    #[test]
    fn await_future_output() {
        let future = async { 123 };

        let result = block_on(async { future.await }, &Config::default());

        assert_eq!(result, 123);
    }

    #[test]
    fn await_task_output() {
        let result = block_on(async { spawn(async { 123 }).await }, &Config::default());

        assert_eq!(result.unwrap(), 123);
    }

    #[test]
    #[ignore]
    // FIXME: just crashes the other thread...
    fn panic_waker_on_another_thread() {
        struct Timer {
            completed: Arc<AtomicBool>,
            waker: Arc<Mutex<Option<Waker>>>,
        }

        impl Timer {
            fn new(delay: Duration) -> Self {
                let completed = Arc::new(AtomicBool::new(false));
                let waker: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));

                let completed_copy = completed.clone();
                let waker_copy = waker.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(delay);
                    completed_copy.store(true, Ordering::SeqCst);
                    let mut guard = waker_copy.lock().unwrap();
                    if let Some(waker) = guard.take() {
                        waker.wake();
                    }
                });

                Timer { completed, waker }
            }
        }

        impl Future for Timer {
            type Output = ();

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                match self.completed.load(Ordering::SeqCst) {
                    true => Poll::Ready(()),
                    false => {
                        let mut guard = self.waker.lock().unwrap();
                        *guard = Some(cx.waker().clone());
                        Poll::Pending
                    }
                }
            }
        }

        block_on(
            async { Timer::new(Duration::from_secs(3)).await },
            &Config::default(),
        );
    }
}
