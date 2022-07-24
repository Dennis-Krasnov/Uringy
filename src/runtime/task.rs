//! Task abstraction for building executors.
//!
//! Inspired by https://docs.rs/async-task/latest/async_task.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

/// ...
pub(crate) fn create<F: Future>(
    future: F,
    schedule: impl Fn(RunHandle, i32, i32),
    runtime_id: i32,
    runtime_fd: i32,
) -> JoinHandle<F::Output> {
    let task = raw::TaskPointer::new(future, schedule, runtime_id, runtime_fd);

    task.schedule();

    JoinHandle {
        task,
        _marker: PhantomData,
    }
}

/// Awaitable handle for the task's output.
///
/// If [`JoinHandle`] is dropped, the task will continue to make progress in the background.
#[derive(Debug)]
pub struct JoinHandle<O> {
    task: raw::TaskPointer,
    _marker: PhantomData<O>,
}

impl<O> Future for JoinHandle<O> {
    type Output = O;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        match self.task.poll_output(context.waker()) {
            Some(output) => Poll::Ready(output),
            None => Poll::Pending,
        }
    }
}

impl<O> Drop for JoinHandle<O> {
    fn drop(&mut self) {
        self.task.decrement_reference_count();
    }
}

/// Handle to a task that exists only when it's ready to run.
///
/// Used within an async runtime to schedule and run tasks.
/// [`TaskHandle`] is devoid of generics for easy storing in collections.
/// Unlike normal dynamic dispatch "fat" pointers, this pointer is one word wide. // TODO: benchmark (maybe more fits into cache)
///
/// If [`TaskHandle`] is dropped, the task won't finish running and will leak resources.
#[derive(Debug)]
pub(crate) struct RunHandle(raw::TaskPointer);

impl RunHandle {
    /// Run the task by polling its future.
    /// Consumes the [`TaskHandle`], it will reappear when the task's waker schedules the task to be run again.
    pub(crate) fn run(self) {
        self.0.run();
    }

    /// ...
    /// for cross-thread.
    /// must be valid pointer.
    pub(crate) unsafe fn from_raw(pointer: *const ()) -> Self {
        RunHandle(raw::TaskPointer::from_raw(pointer))
    }

    /// ...
    /// for cross-thread.
    /// doesn't consume reference count.
    pub(crate) fn to_raw(self) -> *const () {
        let pointer = self.0.as_raw();
        std::mem::forget(self);
        pointer
    }
}

impl Drop for RunHandle {
    fn drop(&mut self) {
        self.0.decrement_reference_count();
    }
}

/// Lower-level task internals.
mod raw {
    use std::future::Future;
    use std::mem::MaybeUninit;
    use std::pin::Pin;
    use std::ptr;
    use std::sync::atomic;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    /// ..
    #[derive(Debug, Clone)]
    pub(super) struct TaskPointer(ptr::NonNull<()>); // TODO: add deref to *const ()

    impl TaskPointer {
        /// ...
        /// setup custom dynamic dispatch to monomorphized functions...
        pub(super) fn new<F: Future, S: Fn(super::RunHandle, i32, i32)>(
            future: F,
            schedule: S,
            runtime_id: i32,
            runtime_fd: i32,
        ) -> Self {
            let task = Box::new(Task {
                // ...
                vtable: TaskVTable {
                    run: do_run::<F, S>,
                    poll_output: do_poll_output::<F, S>,
                    schedule: do_schedule::<F, S>,
                    increment_reference_count: do_increment_reference_count::<F, S>,
                    decrement_reference_count: do_decrement_reference_count::<F, S>,
                },
                state: TaskState {
                    runtime_id,
                    runtime_fd,
                    reference_count: AtomicU32::new(1),
                    future,
                    finished: false,
                    output: MaybeUninit::uninit(),
                    awaiter: None,
                    schedule,
                },
            });

            TaskPointer(ptr::NonNull::new(Box::into_raw(task) as *mut ()).unwrap())
            // TODO: expect
        }

        /// ...
        pub(super) unsafe fn from_raw(pointer: *const ()) -> Self {
            TaskPointer(ptr::NonNull::new(pointer as *mut ()).unwrap()) // TODO: expect
        }

        pub(super) fn as_raw(&self) -> *const () {
            self.0.as_ptr()
        }

        /// ...
        pub(super) fn run(&self) {
            // Safety: ...
            unsafe { ((*self.vtable()).run)(self.clone()) }
        }

        /// ...
        pub(super) fn poll_output<O>(&self, waker: &Waker) -> Option<O> {
            // Safety: ...
            let output = unsafe { ((*self.vtable()).poll_output)(self.clone(), waker) };

            // Safety: ...
            output.map(|pointer| unsafe { (pointer.as_ptr() as *const O).read() })
        }

        /// ...
        pub(super) fn schedule(&self) {
            // Safety: ...
            unsafe { ((*self.vtable()).schedule)(self.clone()) }
        }

        /// ...
        pub(super) fn increment_reference_count(&self) {
            // Safety: ...
            unsafe { ((*self.vtable()).increment_reference_count)(self.clone()) }
        }

        /// ...
        pub(super) fn decrement_reference_count(&self) {
            // Safety: ...
            unsafe { ((*self.vtable()).decrement_reference_count)(self.clone()) }
        }

        fn vtable(&self) -> *const TaskVTable {
            // TODO: document the safety of the cast, in both places
            self.0.as_ptr() as *const TaskVTable
        }

        fn waker(&self) -> Waker {
            const RAW_WAKER_VTABLE: RawWakerVTable =
                RawWakerVTable::new(do_clone_waker, do_wake, do_wake_by_ref, do_drop_waker);

            unsafe fn do_clone_waker(pointer: *const ()) -> RawWaker {
                let task_pointer = TaskPointer::from_raw(pointer);
                task_pointer.increment_reference_count();
                RawWaker::new(pointer, &RAW_WAKER_VTABLE)
            }

            unsafe fn do_wake(pointer: *const ()) {
                let task_pointer = TaskPointer::from_raw(pointer);
                task_pointer.schedule();
                task_pointer.decrement_reference_count();
            }

            unsafe fn do_wake_by_ref(pointer: *const ()) {
                let task_pointer = TaskPointer::from_raw(pointer);
                task_pointer.schedule();
            }

            unsafe fn do_drop_waker(pointer: *const ()) {
                let task_pointer = TaskPointer::from_raw(pointer);
                task_pointer.decrement_reference_count();
            }

            // Safety: All waker invariants are upheld
            unsafe { Waker::from_raw(RawWaker::new(self.as_raw(), &RAW_WAKER_VTABLE)) }
        }
    }

    #[repr(C)]
    struct Task<F: Future, S> {
        // ...
        // The vtable must be the first field in the task memory structure
        // The `#[repr(C)]` memory layout guarantees source-order layout
        vtable: TaskVTable,
        state: TaskState<F, S>,
    }

    struct TaskState<F: Future, S> {
        runtime_id: i32,
        runtime_fd: i32,
        /// built in arc...
        reference_count: AtomicU32,
        // TODO: enum
        future: F,
        finished: bool,
        output: MaybeUninit<F::Output>,
        awaiter: Option<Waker>,
        schedule: S,
    }

    /// ...
    struct TaskVTable {
        /// Run the task by polling its future.
        /// Only call from original thread...
        run: unsafe fn(TaskPointer),

        /// Attempt to resolve future's output.
        /// Returns pointer to output or nullptr if it's not ready yet.
        /// Only call from original thread...
        pub(super) poll_output: unsafe fn(TaskPointer, &Waker) -> Option<ptr::NonNull<()>>,

        /// Schedule this task using the user-specified function.
        pub(super) schedule: unsafe fn(TaskPointer),

        /// ...
        pub(super) increment_reference_count: unsafe fn(TaskPointer),

        /// ...
        pub(super) decrement_reference_count: unsafe fn(TaskPointer),
    }

    unsafe fn do_run<F: Future, S>(task_pointer: TaskPointer) {
        // Safety: only called on one thread...
        let task = &mut *(task_pointer.as_raw() as *mut Task<F, S>);

        // Pin the future to the stack
        // Safety: the future is already allocated on the heap
        let future = Pin::new_unchecked(&mut task.state.future);

        // The [`TaskPointer`] also "implements" the waker interface
        let waker = task_pointer.waker();
        let context = &mut Context::from_waker(&waker);

        task_pointer.increment_reference_count();

        if let Poll::Ready(output) = future.poll(context) {
            task.state.output = MaybeUninit::new(output);
            task.state.finished = true;

            // Notify the waiting join handle
            if let Some(waker) = task.state.awaiter.take() {
                waker.wake();
            }
        }
    }

    unsafe fn do_poll_output<F: Future, S>(
        task_pointer: TaskPointer,
        waker: &Waker,
    ) -> Option<ptr::NonNull<()>> {
        // Safety: only called on one thread...
        let task = &mut *(task_pointer.as_raw() as *mut Task<F, S>);

        if task.state.finished {
            Some(ptr::NonNull::new(task.state.output.as_mut_ptr() as *mut ()).unwrap())
        } else {
            task.state.awaiter = Some(waker.clone());
            None
        }
    }

    unsafe fn do_schedule<F: Future, S: Fn(super::RunHandle, i32, i32)>(task_pointer: TaskPointer) {
        task_pointer.increment_reference_count();

        // do_schedule can be called from any thread, and it takes a &Task<F, S>.
        // do_run and do_poll_output are only called from the original thread, and they take a &mut Task<F, S>.
        // Use ptr::addr_of! to avoid the undefined behaviour caused by holding a & and &mut concurrently.
        let task = task_pointer.as_raw() as *const Task<F, S>;
        let schedule = ptr::addr_of!((*task).state.schedule).read();
        let runtime_id = ptr::addr_of!((*task).state.runtime_id).read();
        let runtime_fd = ptr::addr_of!((*task).state.runtime_fd).read();
        (schedule)(super::RunHandle(task_pointer), runtime_id, runtime_fd);
    }

    unsafe fn do_increment_reference_count<F: Future, S>(task_pointer: TaskPointer) {
        let task = task_pointer.as_raw() as *const Task<F, S>;
        // Safety: ...
        let reference_count = &*ptr::addr_of!((*task).state.reference_count);

        // See https://www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html and Arc docs.
        assert!(reference_count.fetch_add(1, Ordering::Relaxed) > 0);
    }

    unsafe fn do_decrement_reference_count<F: Future, S>(task_pointer: TaskPointer) {
        let task = task_pointer.as_raw() as *const Task<F, S>;
        // Safety: ...
        let reference_count = &*ptr::addr_of!((*task).state.reference_count);

        // See https://www.boost.org/doc/libs/1_55_0/doc/html/atomic/usage_examples.html and Arc docs.
        if reference_count.fetch_sub(1, Ordering::Release) == 1 {
            atomic::fence(Ordering::Acquire);

            // Deallocate task
            drop(Box::from_raw(task_pointer.as_raw() as *mut Task<F, S>));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod join_handle {
        use super::*;

        #[test]
        fn implements_traits() {
            use impls::impls;
            use std::fmt::Debug;

            assert!(impls!(JoinHandle<i32>: Debug & !Send & !Sync & !Clone));
        }

        #[test]
        fn conditionally_implements_debug() {
            use impls::impls;
            use std::fmt::Debug;

            // Given
            struct NotDebug;

            // Then
            assert!(impls!(JoinHandle<NotDebug>: !Debug));
        }
    }

    mod run_handle {
        use super::*;

        #[test]
        fn implements_traits() {
            use impls::impls;
            use std::fmt::Debug;

            assert!(impls!(RunHandle: Debug & !Send & !Sync & !Clone));
        }
    }
}
