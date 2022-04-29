//! Task abstraction for building executors.
//!
//! separate from event loop. somewhat multi-purpose, can be tested independently.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::ptr::NonNull;
use std::task::{Context, Poll};
use std::thread;

/// Awaitable handle for the task's output.
/// TODO: returned from spawn()
///
/// The task's output is wrapped with [`std::thread::Result`] to handle panics within the Future.
///
/// If [`JoinHandle`] is dropped, the task will continue to make progress in the background.
#[derive(Debug)]
pub struct JoinHandle<OUT> {
    /// Pointer to the heap-allocated task.
    /// A void pointer is used to avoid specifying FUT and SCH generics.
    task: NonNull<()>,

    /// Zero-sized marker to get rid of unused generic parameter error.
    _marker: PhantomData<OUT>,
}

// TODO: impl<R> Unpin for JoinHandle<R> {}

// TODO
impl<OUT> Drop for JoinHandle<OUT> {
    fn drop(&mut self) {
        // println!("join handle drop");
    }
}

impl<OUT> Future for JoinHandle<OUT> {
    type Output = thread::Result<OUT>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        // Safety: The task is guaranteed to outlive its handles
        let outcome = unsafe {
            let vtable = raw::vtable(self.task);
            (vtable.poll_output)(self.task, context.waker())
        };

        if outcome.is_null() {
            // TODO: need to save waker and call wake!
            Poll::Pending
        } else {
            let outcome = outcome as *const thread::Result<OUT>;
            // Safety: Outcome is not null and both the handle and the raw task use the same generic type
            let output: thread::Result<OUT> = unsafe { outcome.read() };
            Poll::Ready(output)
        }
    }
}

/// ...
pub(crate) fn create<FUT, SCH>(future: FUT, schedule: SCH) -> JoinHandle<FUT::Output>
where
    FUT: Future,
    SCH: Fn(TaskHandle),
{
    let task = raw::Task::new(future, schedule);
    let raw_box_pointer = Box::into_raw(Box::new(task)); // TODO: garbage collection
    let task_pointer: NonNull<()> = NonNull::new(raw_box_pointer).unwrap().cast();

    // Safety: The task is guaranteed to outlive its handles
    unsafe {
        // Initially schedule the task to be run
        // Must be executed through a heap-based task pointer
        let vtable = raw::vtable(task_pointer);
        (vtable.schedule)(task_pointer);
    }

    JoinHandle {
        task: task_pointer,
        _marker: PhantomData,
    }

    // TODO: arc
    // let task = Arc::new(raw::Task::new(future, schedule));
    // let pointer = Arc::into_raw(task);
}

/// Handle to a task that exists only when it's ready to run.
///
/// Used within an async runtime to schedule and run tasks.
/// [`TaskHandle`] is devoid of generics for easy storing in collections.
/// Unlike normal dynamic dispatch "fat" pointers, this pointer is one word wide. // TODO: benchmark (maybe more fits into cache)
///
/// If [`TaskHandle`] is dropped, the task won't finish running and will leak resources.
#[derive(Debug)]
pub(crate) struct TaskHandle {
    /// Pointer to the heap-allocated task.
    /// A void pointer is used to avoid specifying FUT and SCH generics.
    task: NonNull<()>,
}

impl TaskHandle {
    /// Run the task by polling its future.
    /// Consumes the [`TaskHandle`], it will reappear when the task's waker schedules the task to be run again.
    pub(crate) fn run(self) {
        // Safety: The task is guaranteed to outlive its handles
        unsafe {
            let vtable = raw::vtable(self.task);
            (vtable.run)(self.task);
        };
    }
}

// TODO
impl Drop for TaskHandle {
    fn drop(&mut self) {
        // println!("task handle drop");
    }
}

/// Lower-level task internals.
/// Interfaced through a custom dynamic dispatch mechanism.
mod raw {
    use std::future::Future;
    use std::mem::MaybeUninit;
    use std::panic::AssertUnwindSafe;
    use std::pin::Pin;
    use std::ptr::NonNull;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
    use std::{panic, ptr, thread};

    /// Internal task representation.
    #[repr(C)]
    pub(super) struct Task<FUT: Future, SCH: Fn(super::TaskHandle)> {
        // The vtable must be the first field in the task memory structure
        // The `#[repr(C)]` memory layout guarantees source-order layout
        vtable: &'static TaskVTable,

        // TODO: optimize internal layout (enum)
        future: FUT,

        finished: bool,

        output: MaybeUninit<thread::Result<FUT::Output>>,

        schedule: SCH,

        /// TODO ...
        awaiter: Option<Waker>,
        //     spawned_on: thread::ThreadId,
    }

    /// Retrieves the vtable from a pointer to a [`Task`].
    /// Relies on the [`Task`]'s internal memory layout.
    pub(super) unsafe fn vtable(task: NonNull<()>) -> &'static TaskVTable {
        let vtable_pointer = task.as_ptr() as *mut &'static TaskVTable;
        *vtable_pointer as &'static TaskVTable
    }

    impl<FUT, SCH> Task<FUT, SCH>
    where
        FUT: Future,
        SCH: Fn(super::TaskHandle),
    {
        /// Create a new task from a future and scheduling strategy function.
        pub(super) fn new(future: FUT, schedule: SCH) -> Self {
            // println!("task created from thread {:?}", std::thread::current().id());
            Task {
                // TODO: handle arc in allocate() and in run::<FUT, SCH>, etc.
                // Non-generic vtable function pointers are initialized with monomorphized generic functions
                vtable: &TaskVTable {
                    run: do_run::<FUT, SCH>,
                    poll_output: do_poll_output::<FUT, SCH>,
                    schedule: do_schedule::<FUT, SCH>,
                },
                future,
                finished: false,
                output: MaybeUninit::uninit(),
                schedule,
                awaiter: None,
                // spawned_on: thread::current().id(),
            }
        }
    }

    /// Interface for custom dynamic dispatch.
    /// Callable with void pointers to [`Task`]s (without generics).
    pub(super) struct TaskVTable {
        /// Run the task by polling its future.
        pub(super) run: unsafe fn(NonNull<()>),

        /// Attempt to resolve future's output.
        /// Returns pointer to output or nullptr if it's not ready yet.
        pub(super) poll_output: unsafe fn(NonNull<()>, &Waker) -> *const (),

        /// Schedule this task using the user-specified function.
        pub(super) schedule: unsafe fn(NonNull<()>),
    }

    /// Generics-based implementation of [`run`] in [`TaskVTable`].
    unsafe fn do_run<FUT: Future, SCH: Fn(super::TaskHandle)>(pointer: NonNull<()>) {
        // println!("task run from thread {:?}", std::thread::current().id());

        // Safety: pointer is always a [`Task`]
        let mut task_pointer = pointer.cast::<Task<FUT, SCH>>();
        let task: &mut Task<FUT, SCH> = task_pointer.as_mut();

        // Pin the future to the stack
        // Safety: the future is already allocated on the heap
        let future = Pin::new_unchecked(&mut task.future);

        // The [`Task`] also "implements" the waker interface
        // Safety: All waker invariants are upheld
        let waker = Waker::from_raw(RawWaker::new(pointer.as_ptr(), &RAW_WAKER_VTABLE));
        let context = &mut Context::from_waker(&waker);

        // Panics within futures are caught and handled
        // TODO: is assertunwindsafe even safe to do? https://doc.rust-lang.org/std/panic/struct.AssertUnwindSafe.html
        match panic::catch_unwind(AssertUnwindSafe(|| future.poll(context))) {
            Ok(outcome) => {
                if let Poll::Ready(output) = outcome {
                    task.output = MaybeUninit::new(Ok(output));
                    task.finished = true;

                    // Notify the waiting join handle
                    if let Some(waker) = task.awaiter.take() {
                        waker.wake();
                    }
                }
            }
            Err(err) => {
                task.output = MaybeUninit::new(Err(err));
                task.finished = true;

                // Notify the waiting join handle
                if let Some(waker) = task.awaiter.take() {
                    waker.wake();
                }
            }
        }
        // FIXME: it turns out there's no overhead for catch unwind.
        // if let Poll::Ready(output) = future.poll(context) {
        //     task.output = MaybeUninit::new(Ok(output));
        //     task.finished = true;
        //
        //     // Notify the waiting join handle
        //     if let Some(waker) = task.awaiter.take() {
        //         waker.wake();
        //     }
        // }
    }

    /// Generics-based implementation of [`poll_output`] in [`TaskVTable`].
    unsafe fn do_poll_output<FUT: Future, SCH: Fn(super::TaskHandle)>(
        pointer: NonNull<()>,
        waker: &Waker,
    ) -> *const () {
        // Safety: pointer is always a [`Task`]
        let mut task_pointer = pointer.cast::<Task<FUT, SCH>>();
        let task: &mut Task<FUT, SCH> = task_pointer.as_mut();

        if task.finished {
            task.output.as_ptr() as *const thread::Result<FUT::Output> as *const ()
        } else {
            task.awaiter = Some(waker.clone());
            ptr::null()
        }
    }

    /// Generics-based implementation of [`schedule`] in [`TaskVTable`].
    unsafe fn do_schedule<FUT: Future, SCH: Fn(super::TaskHandle)>(pointer: NonNull<()>) {
        // Safety: pointer is always a [`Task`]
        let mut task_pointer = pointer.cast::<Task<FUT, SCH>>();
        let task: &mut Task<FUT, SCH> = task_pointer.as_mut(); // FIXME: if many wakers wake at once, will be multiple &mut pointers...

        // Safety: TODO
        (task.schedule)(super::TaskHandle { task: pointer });
    }

    // TODO: make sure I never have multiple &mut Tasks!!! (any number of wakers on any number of threads)
    const RAW_WAKER_VTABLE: RawWakerVTable =
        RawWakerVTable::new(clone_waker, wake, wake_by_ref, drop_waker);
    //
    unsafe fn clone_waker(pointer: *const ()) -> RawWaker {
        // TODO: garbage collection
        // println!(
        //     "clone_waker: {pointer:?} from thread {:?}",
        //     std::thread::current().id()
        // );

        //     println!("task clone waker {:?}", pointer);
        //     //     // TODO: I think the solution is to put this waker stuff into the generic impl block for task.
        //     //     // let task: &mut Task<F, S> = pointer.into(); // FIXME: don't have generics here, need to do dynamic dispatch
        //     //     // TODO: increment arc
        //     //     RawWaker::new(pointer.as_ptr(), &Self::RAW_WAKER_VTABLE)
        RawWaker::new(pointer, &RAW_WAKER_VTABLE)
    }

    unsafe fn wake(pointer: *const ()) {
        // TODO: panic if called from another thread
        // TODO: garbage collection
        // println!(
        //     "wake: {pointer:?} from thread {:?}",
        //     std::thread::current().id()
        // );

        let task_pointer = NonNull::new(pointer as *mut ()).unwrap();
        // Safety: The task is guaranteed to outlive its handles
        let vtable = vtable(task_pointer);
        (vtable.schedule)(task_pointer);

        // if same_thread {
        //     let pointer = NonNull::new(pointer as *mut ()).unwrap();
        //     let vtable = vtable(pointer);
        //     (vtable.schedule)(pointer);
        // } else {
        //     // panic?
        //     // TODO: TDD
        // }

        //     println!("task wake {:?}", pointer);
        //     //     let task: &mut Task<FUT, SCH> = pointer.into(); // FIXME: don't have generics here, need to do dynamic dispatch
        //     //
        //     //     // TODO: decrement arc
        //     //     if task.spawned_on == thread::current().id() {
        //     //         // TODO: check that not completed. not closed, not already scheduled
        //     //         task.schedule(super::TaskHandle {
        //     //             task: NonNull::from(pointer),
        //     //         });
        //     //     } else {
        //     //         unimplemented!();
        //     //     }
    }

    unsafe fn wake_by_ref(pointer: *const ()) {
        // TODO: panic if called from another thread
        // println!(
        //     "wake_by_ref: {pointer:?} from thread {:?}",
        //     std::thread::current().id()
        // );

        let task_pointer = NonNull::new(pointer as *mut ()).unwrap();
        // Safety: The task is guaranteed to outlive its handles
        let vtable = vtable(task_pointer);
        (vtable.schedule)(task_pointer);

        //     println!("task wake by ref {:?}", pointer);
        //
        //     //     // let task: &mut Task<F, S> = pointer.into(); // FIXME: don't have generics here, need to do dynamic dispatch
        //     //     let task: &mut Task<FUT, SCH> = pointer.into();
        //     //
        //     //     if task.spawned_on == thread::current().id() {
        //     //         // TODO: check that not completed. not closed, not already scheduled
        //     //         task.schedule(super::TaskHandle {
        //     //             task: NonNull::from(pointer),
        //     //         });
        //     //     } else {
        //     //         unimplemented!();
        //     //     }
    }

    unsafe fn drop_waker(_pointer: *const ()) {
        // TODO: garbage collection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use noop_waker::noop_waker;
    use std::cell::RefCell;

    fn poll<OUT>(join_handle: &mut JoinHandle<OUT>) -> Poll<thread::Result<OUT>> {
        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);
        let join_handle = unsafe { Pin::new_unchecked(join_handle) };
        join_handle.poll(&mut context)
    }

    #[test]
    fn schedule_initially() {
        let scheduled = RefCell::new(false);
        // let mut scheduled = false;

        let _join_handle = create(async {}, |_task_handle| {
            *scheduled.borrow_mut() = true;
        });

        assert!(*scheduled.borrow());
    }

    #[test]
    fn dont_run_initially() {
        let mut ran = false;

        let _join_handle = create(
            async {
                ran = true;
            },
            |_task_handle| {},
        );

        assert!(!ran);
    }

    #[test]
    fn poll_future_on_task_handle_run() {
        let mut ran = false;

        let _join_handle = create(
            async {
                ran = true;
            },
            |task_handle| {
                task_handle.run();
            },
        );

        assert!(ran);
    }

    // #[test]
    // fn pending_initially() {
    //     let (_notifier, mut waiter) = oneshot_notify();
    //
    //     assert!(poll(&mut waiter).is_pending());
    // }
    // TODO: don't initially run schedule?

    #[test]
    fn await_outcome_on_join_handle() {
        let mut join_handle = create(async { 42 }, |task_handle| {
            task_handle.run();
        });

        if let Poll::Ready(output) = poll(&mut join_handle) {
            assert_eq!(output.unwrap(), 42);
        } else {
            panic!("future not ready");
        }
    }

    #[test]
    fn await_error_on_join_handle_panic() {
        let mut join_handle = create(async { panic!("oops") }, |task_handle| {
            task_handle.run();
        });

        if let Poll::Ready(output) = poll(&mut join_handle) {
            assert!(output.is_err());
        } else {
            panic!("future not ready");
        }
    }

    struct ChangeOnDrop(*mut bool);

    impl Drop for ChangeOnDrop {
        fn drop(&mut self) {
            unsafe {
                *self.0 = true;
            }
        }
    }

    #[test]
    #[ignore] // FIXME!
    fn dealloc_output_after_join_handle_drop() {
        let mut deallocated = false;
        let change_on_drop = ChangeOnDrop(&mut deallocated);
        let join_handle = create(async { change_on_drop }, |task_handle| {
            task_handle.run(); // TODO: can't run immediately...
        });

        drop(join_handle);

        assert!(deallocated);
    }

    // struct AFuture {
    //     called: bool,
    // }
    //
    // impl Future for AFuture {
    //     type Output = ();
    //
    //     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    //         println!("poll called! {:?}", self.called);
    //         match self.called {
    //             false => {
    //                 self.called = true;
    //                 cx.waker().wake_by_ref();
    //                 Poll::Pending
    //             }
    //             true => Poll::Ready(()),
    //         }
    //     }
    // }

    // pub async fn yield_now() {
    //     /// Yield implementation
    //     struct YieldNow {
    //         yielded: bool,
    //     }
    //
    //     impl Future for YieldNow {
    //         type Output = ();
    //
    //         fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    //             if self.yielded {
    //                 return Poll::Ready(());
    //             }
    //
    //             self.yielded = true;
    //             cx.waker().wake_by_ref();
    //             Poll::Pending
    //         }
    //     }
    //
    //     YieldNow { yielded: false }.await
    // }

    struct YieldNow {
        yielded: bool,
    }

    impl Future for YieldNow {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if self.yielded {
                return Poll::Ready(());
            }

            self.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    #[test]
    fn wake_calls_schedule() {
        // let mut join_handle = create(async { yield_now().await }, |task_handle| {
        let mut join_handle = create(YieldNow { yielded: false }, |task_handle| {
            task_handle.run();
        });

        // assert!(poll(&mut join_handle).is_pending());
        assert!(poll(&mut join_handle).is_ready());
    }

    // TODO: create a few wakers, test the same thing

    // TODO: drops output if dropped join handle (before and after output is ready)

    // TODO: waker works

    // TODO: cross-thread
}
