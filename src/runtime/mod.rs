use std::collections::{BTreeSet, VecDeque};
use std::num::NonZeroUsize;
use std::{io, marker, mem, panic, thread};

pub(crate) mod context_switch;
mod stack;
mod uring;

use std::cell::UnsafeCell;

thread_local! {
    static RUNTIME: UnsafeCell<Option<RuntimeState>> = UnsafeCell::new(None);
}

pub(crate) fn runtime_exists() -> bool {
    RUNTIME.with(|tls| {
        let runtime = unsafe { &*tls.get() };
        runtime.is_some()
    })
}

/// ...
pub(crate) unsafe fn runtime() -> &'static mut RuntimeState {
    RUNTIME.with(|tls| {
        let borrow = &mut *tls.get();
        borrow.as_mut().unwrap() // TODO: unwrap_unchecked
    })
}

pub(crate) struct RuntimeState {
    uring: uring::Uring,
    pub(crate) fibers: Fibers,
    pub(crate) ready_fibers: VecDeque<FiberIndex>,
    running_fiber: Option<FiberIndex>,
    stack_pool: Vec<*const u8>,
    bootstrap: mem::MaybeUninit<context_switch::Continuation>,
}

impl RuntimeState {
    fn new() -> Self {
        RuntimeState {
            uring: uring::Uring::new(),
            fibers: Fibers::new(),
            ready_fibers: VecDeque::new(),
            running_fiber: None,
            stack_pool: Vec::new(),
            bootstrap: mem::MaybeUninit::uninit(),
        }
    }

    /// Returns bottom of stack...
    fn allocate_stack(&mut self) -> *const u8 {
        if let Some(stack_bottom) = self.stack_pool.pop() {
            return stack_bottom;
        }

        let stack = stack::Stack::new(NonZeroUsize::MIN, NonZeroUsize::new(32).unwrap()).unwrap();
        let stack_base = stack.base();
        mem::forget(stack); // FIXME
        stack_base
    }

    /// ...
    pub(crate) fn running(&self) -> FiberIndex {
        self.running_fiber.unwrap() // TODO: unwrap_unchecked, unsafe fn
    }

    fn process_io(&mut self) {
        for (user_data, result) in self.uring.process_cq() {
            let fiber = FiberIndex(user_data.0 as u32);
            self.fibers.get(fiber).syscall_result = Some(result);
            self.ready_fibers.push_back(fiber);
        }
    }

    /// ...
    pub(crate) fn process_io_and_wait(&mut self) -> FiberIndex {
        loop {
            self.process_io();

            if let Some(fiber) = self.ready_fibers.pop_front() {
                self.running_fiber = Some(fiber);
                break fiber;
            }

            self.uring.wait_for_completed_syscall();
        }
    }
}

impl Drop for RuntimeState {
    fn drop(&mut self) {
        // FIXME: don't hard code, read from runtime
        let guard_pages = 1;
        let usable_pages = 32;

        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
        assert_eq!(page_size, 4096);
        let length = (guard_pages + usable_pages) * page_size;

        for stack_bottom in self.stack_pool.drain(..) {
            let pointer = unsafe { stack_bottom.sub(length) } as *mut u8;
            drop(stack::Stack { pointer, length })
        }
    }
}

pub(crate) struct Fibers(slab::Slab<FiberState>);

impl Fibers {
    fn new() -> Self {
        Fibers(slab::Slab::new())
    }

    pub(crate) fn get(&mut self, fiber: FiberIndex) -> &mut FiberState {
        &mut self.0[fiber.0 as usize] // TODO: get unchecked, unsafe fn
    }

    fn add(
        &mut self,
        parent: Option<FiberIndex>,
        stack_base: *const u8,
        continuation: context_switch::Continuation,
    ) -> FiberIndex {
        let index = self.0.insert(FiberState {
            stack_base,
            continuation,
            is_completed: false,
            join_handle: JoinHandleState::Unused,
            syscall_result: None,
            parent,
            children: BTreeSet::new(),
        });
        FiberIndex(index as u32)
    }

    fn remove(&mut self, fiber: FiberIndex) {
        self.0.remove(fiber.0 as usize);
    }
}

/// ...
/// max 4.3 billion...
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FiberIndex(u32);

#[derive(Debug)]
pub(crate) struct FiberState {
    stack_base: *const u8, // stack grows downwards
    pub(crate) continuation: context_switch::Continuation,
    is_completed: bool,
    join_handle: JoinHandleState,
    syscall_result: Option<io::Result<u32>>,
    parent: Option<FiberIndex>,
    children: BTreeSet<FiberIndex>, // 24B, hashmap is 48B
}

#[derive(Debug, Clone)]
enum JoinHandleState {
    Unused,
    Waiting(FiberIndex), // TODO: rename to Joining?
    Dropped,
}

/// ...
pub fn start<F: FnOnce() -> T, T>(f: F) -> T {
    unsafe {
        exclusive_runtime(|| {
            let stack_base = runtime().allocate_stack();

            let closure_pointer = (stack_base as *mut F).sub(1);
            closure_pointer.write(f);

            let continuation = context_switch::prepare_stack(
                stack_base.sub(closure_union_size::<F, T>()) as *mut u8,
                start_trampoline::<F, T> as *const (),
            );

            let root_fiber = runtime().fibers.add(None, stack_base, continuation);
            runtime().running_fiber = Some(root_fiber);

            let bootstrap = runtime().bootstrap.as_mut_ptr();
            context_switch::jump(continuation, bootstrap); // woken up by root fiber

            let output_pointer = (stack_base as *const thread::Result<T>).sub(1);
            match output_pointer.read() {
                Ok(output) => output,
                Err(e) => panic::resume_unwind(e),
            }
        })
    }
}

unsafe fn exclusive_runtime<T>(f: impl FnOnce() -> T) -> T {
    RUNTIME.with(|tls| {
        let runtime = &mut *tls.get();
        assert!(runtime.is_none());
        *runtime = Some(RuntimeState::new());
    });

    let output = f();

    RUNTIME.with(|tls| {
        let runtime = &mut *tls.get();
        *runtime = None;
    });

    output
}

unsafe extern "C" fn start_trampoline<F: FnOnce() -> T, T>() -> ! {
    let running = runtime().running();
    let stack_base = runtime().fibers.get(running).stack_base;
    let closure_pointer = (stack_base as *const F).sub(1);
    let output_pointer = (stack_base as *mut thread::Result<T>).sub(1);

    // Execute closure
    let closure = closure_pointer.read();
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| (closure)()));
    output_pointer.write(result);
    runtime().fibers.get(running).is_completed = true;

    // Wait for children
    if !runtime().fibers.get(running).children.is_empty() {
        let to = runtime().process_io_and_wait();
        let to = runtime().fibers.get(to).continuation;
        let continuation = &mut runtime().fibers.get(running).continuation;
        context_switch::jump(to, continuation); // woken up by last child
    }

    // Deallocate stack
    runtime().stack_pool.push(stack_base);
    runtime().fibers.remove(running);

    // Return to original thread
    let to = runtime().bootstrap.assume_init();
    let mut dummy = mem::MaybeUninit::uninit();
    unsafe { context_switch::jump(to, dummy.as_mut_ptr()) };
    unreachable!();
}

pub fn yield_now() {
    assert!(runtime_exists());

    unsafe {
        runtime().process_io();

        if runtime().ready_fibers.is_empty() {
            return;
        }

        let running = runtime().running();
        runtime().ready_fibers.push_back(running);
        let to = runtime().ready_fibers.pop_front().unwrap();
        runtime().running_fiber = Some(to);
        let to = runtime().fibers.get(to).continuation;
        let continuation = &mut runtime().fibers.get(running).continuation;
        context_switch::jump(to, continuation); // woken up immediately
    }
}

/// Spawns a new fiber, returning a [`JoinHandle`] for it.
pub fn spawn<F: FnOnce() -> T, T>(f: F) -> JoinHandle<T> {
    assert!(runtime_exists());

    unsafe {
        let stack_base = runtime().allocate_stack();

        let closure_pointer = (stack_base as *mut F).sub(1);
        closure_pointer.write(f);

        let continuation = context_switch::prepare_stack(
            stack_base.sub(closure_union_size::<F, T>()) as *mut u8,
            spawn_trampoline::<F, T> as *const (),
        );

        let parent = runtime().running();
        let child = runtime().fibers.add(Some(parent), stack_base, continuation);
        runtime().fibers.get(parent).children.insert(child);
        runtime().ready_fibers.push_back(child);

        JoinHandle::new(child)
    }
}

unsafe extern "C" fn spawn_trampoline<F: FnOnce() -> T, T>() -> ! {
    let running = runtime().running();
    let stack_base = runtime().fibers.get(running).stack_base;
    let closure_pointer = (stack_base as *const F).sub(1);
    let output_pointer = (stack_base as *mut thread::Result<T>).sub(1);

    // Execute closure
    let closure = closure_pointer.read();
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| (closure)()));
    output_pointer.write(result);
    runtime().fibers.get(running).is_completed = true;

    // Wait for children
    if !runtime().fibers.get(running).children.is_empty() {
        let to = runtime().process_io_and_wait();
        let to = runtime().fibers.get(to).continuation;
        let continuation = &mut runtime().fibers.get(running).continuation;
        context_switch::jump(to, continuation); // woken up by last child
    }

    // Schedule joining fiber
    if let JoinHandleState::Waiting(fiber) = runtime().fibers.get(running).join_handle {
        runtime().ready_fibers.push_back(fiber);
    }

    // Handle parent
    let parent = runtime().fibers.get(running).parent.unwrap();
    runtime().fibers.get(parent).children.remove(&running);

    if runtime().fibers.get(parent).is_completed && runtime().fibers.get(parent).children.is_empty()
    {
        runtime().ready_fibers.push_back(parent);
    }

    // Deallocate stack
    if let JoinHandleState::Dropped = runtime().fibers.get(running).join_handle {
        let stack_base = runtime().fibers.get(running).stack_base;
        runtime().stack_pool.push(stack_base);
    }

    // Continue to next fiber
    let to = runtime().process_io_and_wait();
    let to = runtime().fibers.get(to).continuation;
    let mut dummy = mem::MaybeUninit::uninit();
    unsafe { context_switch::jump(to, dummy.as_mut_ptr()) };
    unreachable!();
}

/// ...
#[derive(Debug)]
pub struct JoinHandle<T> {
    fiber: FiberIndex,
    output: marker::PhantomData<T>,
}

impl<T> JoinHandle<T> {
    fn new(fiber: FiberIndex) -> Self {
        JoinHandle {
            fiber,
            output: marker::PhantomData,
        }
    }

    /// ...
    pub fn join(self) -> thread::Result<T> {
        assert!(runtime_exists());

        unsafe {
            let stack_base = runtime().fibers.get(self.fiber).stack_base;
            let output_pointer = (stack_base as *const thread::Result<T>).sub(1);

            // Already completed
            if runtime().fibers.get(self.fiber).is_completed {
                return output_pointer.read();
            }

            // Wait for completion
            let running = runtime().running();

            runtime().fibers.get(self.fiber).join_handle = JoinHandleState::Waiting(running);

            let to = runtime().process_io_and_wait();
            let to = runtime().fibers.get(to).continuation;
            let continuation = &mut runtime().fibers.get(running).continuation;
            context_switch::jump(to, continuation); // woken up by joined fiber

            assert!(runtime().fibers.get(self.fiber).is_completed);
            output_pointer.read()
        }
    }

    // TODO: cancel
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        assert!(runtime_exists());

        unsafe {
            runtime().fibers.get(self.fiber).join_handle = JoinHandleState::Dropped;

            // Deallocate stack
            if runtime().fibers.get(self.fiber).is_completed {
                let stack_base = runtime().fibers.get(self.fiber).stack_base;
                runtime().stack_pool.push(stack_base);
                runtime().fibers.remove(self.fiber);
            }
        }
    }
}

// TODO: top-level cancel

pub(crate) fn syscall(sqe: io_uring::squeue::Entry) -> io::Result<u32> {
    assert!(runtime_exists());

    unsafe {
        let running = runtime().running();

        assert!(runtime().fibers.get(running).syscall_result.is_none());
        runtime()
            .uring
            .issue_syscall(uring::UserData(running.0 as u64), sqe); // TODO: uring::UserData::from(running)

        let to = runtime().process_io_and_wait();

        if running != to {
            let to = runtime().fibers.get(to).continuation;
            let continuation = &mut runtime().fibers.get(running).continuation;
            context_switch::jump(to, continuation); // woken up by event loop
        }

        runtime().fibers.get(running).syscall_result.take().unwrap()
    }
}

pub fn nop() -> io::Result<()> {
    let result = syscall(io_uring::opcode::Nop::new().build())?;
    assert_eq!(result, 0);
    Ok(())
}

const fn closure_union_size<F: FnOnce() -> T, T>() -> usize {
    let closure_size = mem::size_of::<F>();
    let output_size = mem::size_of::<T>();

    if closure_size > output_size {
        closure_size
    } else {
        output_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    mod start {
        use super::*;
        use std::time::Duration;

        #[test]
        fn returns_output() {
            let output = start(|| 123);

            assert_eq!(output, 123);
        }

        #[test]
        #[should_panic]
        fn rethrows_panic() {
            start(|| panic!("oops"));
        }

        #[test]
        fn works_consecutively() {
            start(|| {});
            start(|| {});
        }

        #[test]
        #[should_panic]
        fn cant_nest() {
            start(|| {
                start(|| {});
            });
        }

        #[test]
        fn works_in_parallel() {
            let handle = thread::spawn(|| {
                start(|| {
                    thread::sleep(Duration::from_millis(2));
                });
            });

            thread::sleep(Duration::from_millis(1));
            start(|| {});

            assert!(handle.join().is_ok());
        }

        #[test]
        fn waits_for_children() {
            static mut VALUE: usize = 0;

            start(|| {
                let handle = spawn(|| unsafe { VALUE += 1 });
                drop(handle);

                let handle = spawn(|| unsafe { VALUE += 1 });
                mem::forget(handle);
            });

            assert_eq!(unsafe { VALUE }, 2);
        }
    }

    mod spawn {
        use super::*;

        #[test]
        fn returns_output() {
            start(|| {
                let handle = spawn(|| 123);

                let output = handle.join();

                assert_eq!(output.unwrap(), 123);
            });
        }

        #[test]
        fn catches_panic() {
            start(|| {
                let result = spawn(|| panic!("oops")).join();
                assert!(result.is_err());
            });
        }

        #[test]
        fn cant_nest_start() {
            start(|| {
                let result = spawn(|| start(|| {})).join();
                assert!(result.is_err());
            });
        }

        #[test]
        fn waits_for_children() {
            start(|| {
                static mut VALUE: usize = 0;

                spawn(|| {
                    let handle = spawn(|| unsafe { VALUE += 1 });
                    drop(handle);

                    let handle = spawn(|| unsafe { VALUE += 1 });
                    mem::forget(handle);
                })
                .join()
                .unwrap();

                assert_eq!(unsafe { VALUE }, 2);
            });
        }
    }

    mod yield_now {
        use super::*;

        #[test]
        fn to_same_fiber() {
            start(|| {
                yield_now();
            });
        }

        #[test]
        fn to_other_fiber() {
            start(|| {
                static mut VALUE: usize = 0;

                spawn(|| unsafe { VALUE += 1 });
                assert_eq!(unsafe { VALUE }, 0);

                yield_now();

                assert_eq!(unsafe { VALUE }, 1);
            });
        }
    }
}
