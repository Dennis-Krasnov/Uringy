//! Async Rust interface for Linux's io_uring.

use crate::runtime::task;
use crate::sync::oneshot_channel;
use crate::utils;
use slab::Slab;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::future::Future;
use std::io;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::sync::atomic::{AtomicI32, Ordering};
use std::task::{Context, Poll};

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
static ID_GENERATOR: AtomicI32 = AtomicI32::new(i32::MAX);

/// An instance of the Uringy runtime, created and destroyed by [`block_on`].
/// Carries the state that's used across [`block_on`], [`schedule`], [`syscall`] functions.
///
/// In async Rust terms, the EventLoop acts as both the Reactor and Executor.
struct EventLoop {
    /// Low-level io_uring instance.
    io_uring: io_uring::IoUring,

    /// Links in-flight syscalls to their awaiting tasks.
    syscall_results: RefCell<Slab<oneshot_channel::Sender<io::Result<u32>>>>,

    /// Queue of tasks that have been scheduled and are ready to be run.
    ///
    /// Unlike Tokio, this isn't a concurrent queue, scheduling a task from another thread requires IPC.
    ready_tasks: RefCell<VecDeque<task::RunHandle>>,

    /// Unique identifier among Uringy runtimes in this process.
    ///
    /// Used to ensure Uringy resources are used on the same runtime they were created on.
    /// Used to determine whether task scheduling is done on the current runtime or requires IPC.
    runtime_id: i32,
}

impl EventLoop {
    /// Creates a new instance of the Uringy runtime.
    fn new(config: &Config) -> Self {
        let io_uring = io_uring::IoUring::new(config.sq_size as u32).expect("io_uring creation");
        assert!(io_uring.params().is_feature_nodrop());

        let runtime_id = ID_GENERATOR.fetch_sub(1, Ordering::SeqCst);

        EventLoop {
            io_uring,
            syscall_results: RefCell::new(Slab::with_capacity(config.sq_size * 8)),
            ready_tasks: RefCell::new(VecDeque::with_capacity(1024)),
            runtime_id,
        }
    }

    /// Polls the [`future`] until completion, while multitasking background tasks.
    ///
    /// This is a separate function from [`block_on`] since it doesn't concern itself with thread local state. // TODO: same with other global functions
    fn run_to_completion<OUT>(&self, future: impl Future<Output = OUT> + 'static) -> OUT {
        // The runtime treats the original future like any other task.
        let mut future_output = spawn(future);

        // Avoid borrow_mut living for the duration of the while loop's body
        // If this closure is inlined, a call to [`spawn()`] during [`task.run()`] would cause a double mutable borrow
        let next_ready_task = || self.ready_tasks.borrow_mut().pop_front();

        loop {
            while let Some(task) = next_ready_task() {
                task.run();
            }

            if self.process_completion_queue() > 0 {
                continue;
            }

            if let Poll::Ready(output) = utils::poll(&mut future_output) {
                // Perform last-minute IO for un-awaited syscalls in the original future
                self.io_uring.submit().unwrap();

                return output;
            }

            // Block the thread until a syscall completes
            self.io_uring
                .submit_and_wait(1)
                .expect("io_uring blocking submit");
        }
    }

    /// ...
    /// to be used in run_to_completion and potentially in submit (ebusy with no_drop)
    fn process_completion_queue(&self) -> usize {
        // Safety: No other completion queue exists
        let cq = unsafe { self.io_uring.completion_shared() };
        let processed = cq.len();

        for cqe in cq {
            // CQE was a msg_ring from a waker in another thread
            if cqe.result() == self.runtime_id {
                assert_eq!(cqe.flags(), 0);

                // Safety: ...
                let run_handle = unsafe { task::RunHandle::from_raw(cqe.user_data() as *const ()) };
                self.ready_tasks.borrow_mut().push_back(run_handle);

                continue;
            }

            // For every completed syscall, sends its result to the awaiting task.
            if let Some(sender) = self
                .syscall_results
                .borrow_mut()
                .try_remove(cqe.user_data() as usize)
            {
                let result = if cqe.result() >= 0 {
                    Ok(cqe.result() as u32)
                } else {
                    Err(io::Error::from_raw_os_error(-cqe.result()))
                };

                let mut send = sender.send(result);
                assert!(utils::poll(&mut send).is_ready());
            } else {
                // Message was meant for another runtime
            }
        }

        processed
    }
}

/// ...
/// Blocks the current thread on a future, processing I/O events when idle. ???
/// When the original future completes, the other tasks are cancelled.
pub fn block_on<OUT>(future: impl Future<Output = OUT> + 'static, config: &Config) -> OUT {
    LOCAL_RUNTIME.with(|local_runtime| {
        // Immutable borrow because block_on may be attempted to run within another block_on.
        if local_runtime.borrow().is_some() {
            panic!(
                "Nested block_on is forbidden, consider spawning a task for the future instead."
            );
        }

        let event_loop = EventLoop::new(config);
        *local_runtime.borrow_mut() = Some(event_loop);

        let output = local_runtime
            .borrow()
            .as_ref()
            .unwrap()
            .run_to_completion(future);

        let event_loop = local_runtime.borrow_mut().take().unwrap();
        drop(event_loop);

        output
    })
}

/// Spawn an asynchronous task onto the event loop.
pub fn spawn<OUT>(future: impl Future<Output = OUT> + 'static) -> task::JoinHandle<OUT> {
    // ...
    fn schedule(task: task::RunHandle, runtime_id: i32, runtime_fd: i32) {
        // Skirt lifetime issues...
        let task_raw_pointer = task.to_raw();

        // ...
        // don't await the syscall, result doesn't matter... need last minute submit in run_to_completion...
        let do_syscall = move || {
            syscall(
                io_uring::opcode::MsgRing::new(
                    runtime_fd,
                    runtime_id as u32,
                    task_raw_pointer as u64,
                )
                .build(),
            );
        };

        LOCAL_RUNTIME.with(|local_runtime| {
            match local_runtime.borrow().as_ref() {
                Some(event_loop) => {
                    if event_loop.runtime_id == runtime_id {
                        // Safety: ...
                        let run_handle = unsafe { task::RunHandle::from_raw(task_raw_pointer) };
                        event_loop.ready_tasks.borrow_mut().push_back(run_handle);
                    } else {
                        do_syscall();
                    }
                }
                None => {
                    block_on(
                        async move {
                            do_syscall();
                        },
                        &Config::default(),
                    );
                }
            }
        });
    }

    LOCAL_RUNTIME.with(|local_runtime| {
        match local_runtime.borrow().as_ref() {
            Some(event_loop) => task::create(future, schedule, event_loop.runtime_id, event_loop.io_uring.as_raw_fd() as i32),
            None => panic!("There's no uringy runtime to spawn the task on, consider blocking on the future instead."),
        }
    })
}

/// ...
/// should really be unsafe!
pub(crate) fn syscall(entry: io_uring::squeue::Entry) -> Syscall {
    // Use channel to ... wait for the result of the syscall
    let (s, r) = oneshot_channel::oneshot_channel();

    let _user_data = LOCAL_RUNTIME.with(|local_runtime| {
        // TODO: defensive, expect with error message?
        match local_runtime.borrow().as_ref() {
            Some(event_loop) => {
                let key = event_loop.syscall_results.borrow_mut().insert(s) as u64;

                // Safety: No other submission queue exists
                let mut sq = unsafe { event_loop.io_uring.submission_shared() };

                let entry = entry.user_data(key);

                while sq.is_full() {
                    event_loop.io_uring.submit().unwrap();
                    sq.sync();
                }

                // Safety: The submission queue isn't full
                unsafe {
                    sq.push(&entry).unwrap();
                }

                key
            }
            None => panic!("WARNING: TCP SEND CALLED BUT EVENT LOOP DOESN'T EXIST!!!"),
        }
    });

    Syscall {
        // user_data,
        receiver: r,
    }
}

pub(crate) struct Syscall {
    // user_data: u64,
    receiver: oneshot_channel::Receiver<io::Result<u32>>,
}

impl Syscall {
    // pub(crate) fn user_data(&self) -> u64 {
    //     self.user_data
    // }
}

impl Future for Syscall {
    type Output = io::Result<u32>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        // Safety: ...
        let receiver = unsafe { Pin::new_unchecked(&mut self.receiver) };
        receiver.poll(context).map(Option::unwrap)
    }
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
    sq_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config { sq_size: 4096 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};
    use std::time::{Duration, Instant};

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

        assert_eq!(result, 123);
    }

    #[test]
    #[ignore] // The CI server isn't running a modern enough Linux kernel
    fn waker_on_another_thread() {
        // TODO: just use an async_channel instead!
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
            async {
                // Given
                let before = Instant::now();

                // When
                Timer::new(Duration::from_millis(5)).await;

                // Then
                assert!(before.elapsed() >= Duration::from_millis(5));
            },
            &Config::default(),
        );
    }
}
