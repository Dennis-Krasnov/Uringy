//! ...

use std::any::Any;
use std::collections::{BTreeSet, VecDeque};
use std::num::NonZeroUsize;
use std::{ffi, hint, io, marker, mem, panic, thread};

mod context_switch;
mod stack;
mod syscall;
mod tls;

/// ...
pub fn start<F: FnOnce() -> T, T>(f: F) -> thread::Result<T> {
    tls::exclusive_runtime(|| {
        let (original, root) = tls::runtime(|runtime| {
            let root_fiber = runtime.create_fiber(f, start_trampoline::<F, T>, false);
            runtime.running_fiber = Some(root_fiber);

            (
                runtime.original.as_mut_ptr(),
                &runtime.running().continuation as *const context_switch::Continuation,
            )
        });

        unsafe { context_switch::jump(original, root) };
        tls::runtime(|rt| unsafe { rt.running().stack.union_ref::<thread::Result<T>>().read() })
    })
}

extern "C" fn start_trampoline<F: FnOnce() -> T, T>() -> ! {
    // execute closure
    let closure: F = tls::runtime(|runtime| {
        let fiber = runtime.running();
        unsafe { fiber.stack.union_ref::<F>().read() }
    });

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| (closure)()));
    hint::black_box(&result); // removing this causes a segfault in release mode

    tls::runtime(|runtime| {
        let fiber = runtime.running();
        fiber.is_completed = true;
        fiber.is_cancelled = true; // prevent cancel scheduling while waiting for children
        unsafe { fiber.stack.union_mut::<thread::Result<T>>().write(result) };
    });

    // wait for children
    if tls::runtime(|rt| !rt.running().children.is_empty()) {
        park(|_| {}); // woken up by last child
    }

    // deallocate stack
    tls::runtime(|runtime| {
        let stack = runtime.running().stack;
        runtime.stack_pool.push(stack);
    });

    // return to original thread
    let mut dummy = mem::MaybeUninit::uninit();
    let original = tls::runtime(|runtime| runtime.original.as_ptr());
    unsafe { context_switch::jump(dummy.as_mut_ptr(), original) };
    unreachable!();
}

struct RuntimeState {
    kernel: syscall::Interface,
    fibers: slab::Slab<FiberState>,
    ready_fibers: VecDeque<FiberIndex>,
    running_fiber: Option<FiberIndex>,
    stack_pool: Vec<StackBase>,
    original: mem::MaybeUninit<context_switch::Continuation>,
}

impl RuntimeState {
    fn new() -> Self {
        RuntimeState {
            kernel: syscall::Interface::new(),
            fibers: slab::Slab::new(),
            ready_fibers: VecDeque::new(),
            running_fiber: None,
            stack_pool: Vec::new(),
            original: mem::MaybeUninit::uninit(),
        }
    }

    fn create_fiber<F: FnOnce() -> T, T>(
        &mut self,
        f: F,
        trampoline: extern "C" fn() -> !,
        is_cancelled: bool,
    ) -> FiberIndex {
        // allocate stack
        let mut stack_base = self.stack_pool.pop().unwrap_or_else(|| {
            let usable_pages = NonZeroUsize::new(32).unwrap();
            let stack = stack::Stack::new(NonZeroUsize::MIN, usable_pages).unwrap();
            let stack_base = StackBase(stack.base());
            mem::forget(stack);
            stack_base
        });

        unsafe { stack_base.union_mut::<F>().write(f) };

        let index = self.fibers.insert(FiberState {
            stack: stack_base,
            continuation: unsafe {
                context_switch::prepare_stack(stack_base.after_union::<F, T>(), trampoline)
            },
            join_handle: JoinHandleState::Unused,
            parent: None,
            children: BTreeSet::new(),
            syscall_result: None,
            is_completed: false,
            is_cancelled,
            // is_scheduled: false,
        });

        FiberIndex(index)
    }

    fn running(&mut self) -> &mut FiberState {
        // TODO: #[cfg(not(debug_assertions))]: unwrap_unchecked, get_unchecked. document performance difference.
        let fiber_index = self.running_fiber.expect("...");
        &mut self.fibers[fiber_index.0]
    }

    fn process_io(&mut self) -> *const context_switch::Continuation {
        loop {
            for (user_data, result) in self.kernel.process_completed() {
                let fiber = FiberIndex(user_data.0 as usize);
                self.fibers[fiber.0].syscall_result = Some(result);
                Waker(fiber).schedule_with(self);
            }

            if let Some(fiber) = self.ready_fibers.pop_front() {
                self.running_fiber = Some(fiber);
                // self.fibers[fiber.0].is_scheduled = false;
                break &self.fibers[fiber.0].continuation as *const context_switch::Continuation;
            }

            self.kernel.wait_for_completed();
        }
    }

    fn cancel(&mut self, root: FiberIndex) {
        // TODO: if is_cancelled { return } (short circuit)

        if !self.fibers[root.0].is_cancelled && root != self.running_fiber.unwrap() {
            Waker(root).schedule_with(self);
        }

        self.fibers[root.0].is_cancelled = true;

        for child in self.fibers[root.0].children.clone() {
            self.cancel(child);
        }
    }

    // TODO: is_contained flag
    fn nearest_contained(&self, _fiber: FiberIndex) -> FiberIndex {
        FiberIndex(0)
    }
}

impl Drop for RuntimeState {
    fn drop(&mut self) {
        let guard_pages = 1;
        let usable_pages = 32;

        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
        let length = (guard_pages + usable_pages) * page_size;

        for stack_bottom in self.stack_pool.drain(..) {
            let pointer = unsafe { stack_bottom.0.byte_sub(length) };
            drop(stack::Stack { pointer, length })
        }
    }
}

/// ...
/// max 4.3 billion... (u32 takes up less space in FiberState)
#[repr(transparent)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FiberIndex(usize);

#[derive(Debug)]
struct FiberState {
    stack: StackBase,
    continuation: context_switch::Continuation,
    join_handle: JoinHandleState,
    parent: Option<FiberIndex>,
    children: BTreeSet<FiberIndex>,
    syscall_result: Option<i32>,
    is_completed: bool,
    is_cancelled: bool,
}

#[derive(Debug)]
enum JoinHandleState {
    Unused,
    Waiting(Option<Waker>), // option for taking ownership from mutable reference
    Dropped,
}

/// Upper address of a fiber's stack memory, stack addresses grow downwards.
/// The union of the user's closure and its output is stored at the top of the stack to save space in [FiberState].
#[derive(Debug, Copy, Clone)]
struct StackBase(*mut ffi::c_void);

impl StackBase {
    unsafe fn union_ref<U>(&self) -> *const U {
        (self.0 as *const U).sub(1)
    }

    unsafe fn union_mut<U>(&mut self) -> *mut U {
        (self.0 as *mut U).sub(1)
    }

    unsafe fn after_union<F, T>(&self) -> *mut ffi::c_void {
        let union_size = std::cmp::max(mem::size_of::<F>(), mem::size_of::<T>());
        self.0.byte_sub(union_size)
    }
}

/// Spawns a new fiber, returning a [JoinHandle] for it.
pub fn spawn<F: FnOnce() -> T + 'static, T: 'static>(f: F) -> JoinHandle<T> {
    let child_fiber = tls::runtime(|runtime| {
        let is_cancelled = runtime.running().is_cancelled;
        let child_fiber = runtime.create_fiber(f, spawn_trampoline::<F, T>, is_cancelled);
        runtime.ready_fibers.push_back(child_fiber);
        // runtime.fibers[child_fiber.0].is_scheduled = true;

        // parent child relationship
        runtime.running().children.insert(child_fiber);
        runtime.fibers[child_fiber.0].parent = Some(runtime.running_fiber.unwrap());

        child_fiber
    });

    JoinHandle::new(child_fiber)
}

extern "C" fn spawn_trampoline<F: FnOnce() -> T, T>() -> ! {
    // execute closure
    let closure: F = tls::runtime(|rt| unsafe { rt.running().stack.union_ref::<F>().read() });
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| (closure)()));
    hint::black_box(&result); // removing this causes a segfault in release mode
    let result_is_error = result.is_err();

    tls::runtime(|runtime| {
        let fiber = runtime.running();

        fiber.is_completed = true;
        fiber.is_cancelled = true; // prevent cancel scheduling while waiting for children
        unsafe { fiber.stack.union_mut::<thread::Result<T>>().write(result) };
    });

    // wait for children
    if tls::runtime(|rt| !rt.running().children.is_empty()) {
        park(|_| {}); // woken up by last child
    }

    // schedule joining fiber
    tls::runtime(|runtime| {
        if let JoinHandleState::Waiting(waker) = &mut runtime.running().join_handle {
            waker.take().unwrap().schedule_with(runtime);
        } else if result_is_error {
            let nearest_contained = runtime.nearest_contained(runtime.running_fiber.unwrap());
            runtime.cancel(nearest_contained);
        }
    });

    // cleanup parent
    tls::runtime(|runtime| {
        let parent_index = runtime.running().parent.unwrap();
        let parent = &mut runtime.fibers[parent_index.0];

        parent.children.remove(&runtime.running_fiber.unwrap());

        if parent.is_completed && parent.children.is_empty() {
            Waker(parent_index).schedule_with(runtime);
        }
    });

    // deallocate stack
    tls::runtime(|runtime| {
        if let JoinHandleState::Dropped = runtime.running().join_handle {
            let stack = runtime.running().stack;
            runtime.stack_pool.push(stack);
            runtime.fibers.remove(runtime.running_fiber.unwrap().0);
        }
    });

    // continue to next fiber
    let mut dummy = mem::MaybeUninit::uninit();
    let next = tls::runtime(|runtime| runtime.process_io());
    unsafe { context_switch::jump(dummy.as_mut_ptr(), next) };
    unreachable!()
}

/// Handle for joining or cancelling a fiber.
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
    /// if cancelled: returns if completes, waits only if waiting on an already cancelled fiber.
    pub fn join(self) -> Result<T, crate::Error<Box<dyn Any + Send + 'static>>> {
        if tls::runtime(|rt| rt.fibers[self.fiber.0].is_completed) {
            return self.read_output();
        }

        if is_cancelled() && !tls::runtime(|rt| rt.fibers[self.fiber.0].is_cancelled) {
            return Err(crate::Error::Cancelled);
        }

        park(|waker| {
            tls::runtime(|runtime| {
                let fiber = &mut runtime.fibers[self.fiber.0];
                assert!(!fiber.is_completed);
                fiber.join_handle = JoinHandleState::Waiting(Some(waker));
            });
        }); // woken up by completion or cancellation

        if tls::runtime(|rt| rt.fibers[self.fiber.0].is_completed) {
            return self.read_output();
        }

        assert!(is_cancelled());
        if !tls::runtime(|rt| rt.fibers[self.fiber.0].is_cancelled) {
            return Err(crate::Error::Cancelled);
        }
        park(|_| {}); // woken up by completion

        self.read_output()
    }

    fn read_output(self) -> Result<T, crate::Error<Box<dyn Any + Send>>> {
        tls::runtime(|runtime| {
            let fiber = &mut runtime.fibers[self.fiber.0];
            let result = unsafe { fiber.stack.union_ref::<thread::Result<T>>().read() };
            result.map_err(|e| crate::Error::Original(e))
        })
    }

    /// ...
    pub fn cancel(&self) {
        tls::runtime(|runtime| {
            runtime.cancel(self.fiber);
        })
    }

    /// ...
    pub fn cancel_propagating(&self) {
        tls::runtime(|runtime| {
            let nearest_contained = runtime.nearest_contained(self.fiber);
            runtime.cancel(nearest_contained);
        })
    }
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        // deallocate stack
        tls::runtime(|runtime| {
            runtime.fibers[self.fiber.0].join_handle = JoinHandleState::Dropped;

            if runtime.fibers[self.fiber.0].is_completed {
                let stack = runtime.fibers[self.fiber.0].stack;
                runtime.stack_pool.push(stack);
                runtime.fibers.remove(self.fiber.0);
            }
        });
    }
}

/// ...
pub fn park(schedule: impl FnOnce(Waker)) {
    let running = tls::runtime(|runtime| runtime.running_fiber.unwrap());

    let waker = Waker(running);
    schedule(waker);

    // continue to next fiber
    let (running, next) = tls::runtime(|runtime| {
        (
            &mut runtime.running().continuation as *mut context_switch::Continuation,
            runtime.process_io(),
        )
    });
    unsafe { context_switch::jump(running, next) };
}

/// Handle for scheduling a parked fiber.
#[repr(transparent)]
#[derive(Debug)]
pub struct Waker(FiberIndex);

impl Waker {
    /// Wake up the parked fiber to be run at some point.
    pub fn schedule(self) {
        tls::runtime(|runtime| {
            self.schedule_with(runtime);
        });
    }

    fn schedule_with(self, runtime: &mut RuntimeState) {
        // if !runtime.fibers[self.0 .0].is_scheduled {
        // FIXME: slow
        if !runtime.ready_fibers.contains(&self.0) {
            runtime.ready_fibers.push_back(self.0);
            // runtime.fibers[self.0 .0].is_scheduled = true;
        }
    }

    // Wake up the parked fiber to be run next.
    // pub fn schedule_immediately(self) {}
}

pub fn yield_now() {
    // TODO: schedule_io, fast path, document speedup

    park(|waker| waker.schedule());
}

/// ...
pub fn cancel() {
    tls::runtime(|runtime| {
        runtime.cancel(runtime.running_fiber.unwrap());
    })
}

/// ...
pub fn cancel_propagating() {
    tls::runtime(|runtime| {
        let nearest_contained = runtime.nearest_contained(runtime.running_fiber.unwrap());
        runtime.cancel(nearest_contained);
    })
}

/// ...
pub fn is_cancelled() -> bool {
    tls::runtime(|runtime| {
        let fiber = runtime.running();
        fiber.is_cancelled
    })
}

pub(crate) fn syscall(sqe: io_uring::squeue::Entry) -> crate::IoResult<u32> {
    if is_cancelled() {
        return Err(crate::Error::Cancelled);
    }

    let fiber_id = tls::runtime(|rt| rt.running_fiber.unwrap());
    let syscall_id = syscall::Id(fiber_id.0 as u64);

    tls::runtime(|runtime| {
        let fiber = runtime.running();
        assert!(fiber.syscall_result.is_none());

        runtime.kernel.issue(syscall_id, sqe);
    });

    park(|_| {}); // woken up by CQE or cancellation

    if tls::runtime(|rt| rt.running().syscall_result.is_some()) {
        return read_syscall_result();
    }

    assert!(is_cancelled());
    tls::runtime(|rt| rt.kernel.cancel(syscall_id));
    park(|_| {}); // woken up by CQE

    read_syscall_result()
}

fn read_syscall_result() -> crate::IoResult<u32> {
    let result = tls::runtime(|rt| rt.running().syscall_result.take()).unwrap();

    if result >= 0 {
        Ok(result as u32)
    } else {
        if -result == libc::ECANCELED {
            return Err(crate::Error::Cancelled);
        }

        let error = io::Error::from_raw_os_error(-result);
        Err(crate::Error::Original(error))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    mod start {
        use std::time::Duration;

        use super::*;

        #[test]
        fn returns_output() {
            let output = start(|| 123);

            assert_eq!(output.unwrap(), 123);
        }

        #[test]
        fn catches_panic() {
            let result = start(|| panic!());

            assert!(result.is_err());
        }

        #[test]
        #[should_panic]
        fn cant_nest() {
            start(|| {
                start(|| {}).unwrap();
            })
            .unwrap();
        }

        #[test]
        fn works_several_times() {
            start(|| {}).unwrap();
            start(|| {}).unwrap();
        }

        #[test]
        fn works_in_parallel() {
            let handle = thread::spawn(|| {
                start(|| thread::sleep(Duration::from_millis(2))).unwrap();
            });

            start(|| {
                thread::sleep(Duration::from_millis(1));
            })
            .unwrap();

            handle.join().unwrap();
        }

        #[test]
        fn waits_for_dropped_child() {
            let before = Instant::now();

            start(|| {
                let handle = spawn(|| crate::time::sleep(Duration::from_millis(5)));
                drop(handle);
            })
            .unwrap();

            assert!(before.elapsed() > Duration::from_millis(5));
        }

        #[test]
        fn waits_for_forgotten_child() {
            let before = Instant::now();

            start(|| {
                let handle = spawn(|| crate::time::sleep(Duration::from_millis(5)));
                mem::forget(handle);
            })
            .unwrap();

            assert!(before.elapsed() > Duration::from_millis(5));
        }

        #[test]
        #[ignore]
        fn cleans_up_after_itself() {
            // enough to hit OS limits
            for _ in 0..1_000_000 {
                start(|| {}).unwrap();
            }
        }

        mod cancellation {
            use super::*;

            #[test]
            fn initially_not_cancelled() {
                start(|| {
                    assert!(!is_cancelled());
                })
                .unwrap();
            }

            #[test]
            fn cancelled_after_cancelling_self() {
                start(|| {
                    cancel();
                    assert!(is_cancelled());
                })
                .unwrap();
            }
        }
    }

    mod spawn {
        use super::*;

        #[test]
        fn returns_child_output() {
            start(|| {
                let handle = spawn(|| 123);

                let output = handle.join();

                assert_eq!(output.unwrap(), 123);
            })
            .unwrap();
        }

        #[test]
        fn returns_non_child_output() {
            start(|| {
                let other = spawn(|| 123);
                let handle = spawn(|| other.join().unwrap());

                let output = handle.join();

                assert_eq!(output.unwrap(), 123);
            })
            .unwrap();
        }

        #[test]
        fn returns_already_completed_output() {
            start(|| {
                let handle = spawn(|| 123);

                yield_now();
                // TODO: assert!(handle.is_completed());
                let output = handle.join();

                assert_eq!(output.unwrap(), 123);
            })
            .unwrap();
        }

        #[test]
        fn catches_panic() {
            start(|| {
                let result = spawn(|| panic!()).join();

                assert!(result.is_err());
            })
            .unwrap();
        }

        #[test]
        fn waits_for_dropped_child() {
            start(|| {
                let handle = spawn(|| {
                    let handle = spawn(|| crate::time::sleep(Duration::from_millis(5)));
                    drop(handle);
                });

                let before = Instant::now();
                handle.join().unwrap();

                assert!(before.elapsed() > Duration::from_millis(5));
            })
            .unwrap();
        }

        #[test]
        fn waits_for_forgotten_child() {
            start(|| {
                let handle = spawn(|| {
                    let handle = spawn(|| crate::time::sleep(Duration::from_millis(5)));
                    mem::forget(handle);
                });

                let before = Instant::now();
                handle.join().unwrap();

                assert!(before.elapsed() > Duration::from_millis(5));
            })
            .unwrap();
        }

        #[test]
        #[ignore]
        fn joined_child_reuses_stack() {
            start(|| {
                // enough to hit OS limits
                for _ in 0..1_000_000 {
                    spawn(|| {}).join().unwrap();
                }
            })
            .unwrap();
        }

        #[test]
        #[ignore]
        fn dropped_child_reuses_stack() {
            start(|| {
                // enough to hit OS limits
                for _ in 0..1_000_000 {
                    let handle = spawn(|| {});
                    drop(handle);
                    yield_now();
                }
            })
            .unwrap();
        }

        #[test]
        #[should_panic]
        #[ignore]
        fn forgotten_child_cant_reuse_stack() {
            start(|| {
                // enough to hit OS limits
                for _ in 0..1_000_000 {
                    let handle = spawn(|| {});
                    mem::forget(handle); // memory leak
                    yield_now();
                }
            })
            .unwrap();
        }

        #[test]
        #[ignore]
        fn joined_child_cleans_up_after_itself() {
            // enough to hit OS limits
            for _ in 0..1_000_000 {
                start(|| {
                    spawn(|| {}).join().unwrap();
                })
                .unwrap();
            }
        }

        #[test]
        #[ignore]
        fn dropped_child_cleans_up_after_itself() {
            // enough to hit OS limits
            for _ in 0..1_000_000 {
                start(|| {
                    let handle = spawn(|| {});
                    drop(handle);
                })
                .unwrap();
            }
        }

        #[test]
        #[should_panic]
        #[ignore]
        fn forgotten_child_cant_clean_up_after_itself() {
            // enough to hit OS limits
            for _ in 0..1_000_000 {
                start(|| {
                    let handle = spawn(|| {});
                    mem::forget(handle); // memory leak
                })
                .unwrap();
            }
        }

        mod cancellation {
            use super::*;

            #[test]
            fn child_initially_not_cancelled() {
                start(|| {
                    let handle = spawn(|| assert!(!is_cancelled()));

                    handle.join().unwrap();
                })
                .unwrap();
            }

            #[test]
            fn child_starts_cancelled_if_parent_cancelled() {
                start(|| {
                    cancel();

                    let handle = spawn(|| assert!(is_cancelled()));

                    handle.join().unwrap();
                })
                .unwrap();
            }

            #[test]
            fn child_cancelled_after_cancelling_self() {
                start(|| {
                    let handle = spawn(|| {
                        cancel();
                        assert!(is_cancelled());
                    });

                    handle.join().unwrap();
                    assert!(!is_cancelled());
                })
                .unwrap();
            }

            #[test]
            fn parent_propagates_cancel_to_children() {
                start(|| {
                    let handle = spawn(|| assert!(is_cancelled()));

                    cancel();

                    handle.join().unwrap();
                })
                .unwrap();
            }

            #[test]
            fn child_cancelled_after_cancelling_handle() {
                start(|| {
                    let handle = spawn(|| assert!(is_cancelled()));

                    handle.cancel();

                    handle.join().unwrap();
                    assert!(!is_cancelled());
                })
                .unwrap();
            }

            #[test]
            fn cancelled_after_child_cancel_propagating_self() {
                start(|| {
                    let handle = spawn(|| {
                        cancel_propagating();
                        assert!(is_cancelled());
                    });

                    handle.join().unwrap();
                    assert!(is_cancelled());
                })
                .unwrap();
            }

            #[test]
            fn cancelled_after_cancel_propagating_handle() {
                start(|| {
                    let handle = spawn(|| assert!(is_cancelled()));

                    handle.cancel_propagating();

                    handle.join().unwrap();
                    assert!(is_cancelled());
                })
                .unwrap();
            }

            #[test]
            fn not_cancelled_after_joined_child_panic() {
                start(|| {
                    let handle = spawn(|| panic!());

                    let _ = handle.join();

                    assert!(!is_cancelled());
                })
                .unwrap();
            }

            #[test]
            fn cancelled_after_dropped_child_panic() {
                start(|| {
                    let handle = spawn(|| panic!());
                    drop(handle);

                    yield_now();

                    assert!(is_cancelled());
                })
                .unwrap();
            }

            #[test]
            fn cancelled_after_forgotten_child_panic() {
                start(|| {
                    let handle = spawn(|| panic!());
                    mem::forget(handle);

                    yield_now();

                    assert!(is_cancelled());
                })
                .unwrap();
            }
        }

        mod syscall {
            use super::*;

            #[test]
            fn executes_syscalls_in_start() {
                start(|| {
                    nop().unwrap();
                    nop().unwrap();
                })
                .unwrap();
            }

            #[test]
            fn executes_syscalls_in_spawn() {
                start(|| {
                    spawn(|| {
                        nop().unwrap();
                        nop().unwrap();
                    })
                    .join()
                    .unwrap();
                })
                .unwrap();
            }

            fn nop() -> crate::CancellableResult<()> {
                let sqe = io_uring::opcode::Nop::new().build();
                let result = syscall(sqe).map_err(|cancellable| cancellable.map(|_| ()))?;
                assert_eq!(result, 0);

                Ok(())
            }

            mod cancellation {
                use super::*;

                #[test]
                fn tries_to_stop_active_syscall() {
                    start(|| {
                        let handle = spawn(|| crate::time::sleep(Duration::from_millis(5)));
                        yield_now();

                        handle.cancel();
                        let before = Instant::now();
                        let result = handle.join().unwrap();

                        assert_eq!(result, Err(crate::Error::Cancelled));
                        assert!(before.elapsed() < Duration::from_millis(5));
                    })
                    .unwrap();
                }

                #[test]
                fn immediately_fails_new_syscall() {
                    start(|| {
                        cancel();

                        let before = Instant::now();
                        let result = crate::time::sleep(Duration::from_millis(5));

                        assert_eq!(result, Err(crate::Error::Cancelled));
                        assert!(before.elapsed() < Duration::from_millis(5));
                    })
                    .unwrap();
                }
            }
        }
    }

    mod yield_now {
        use std::cell::RefCell;
        use std::rc::Rc;

        use super::*;

        #[test]
        fn to_same_fiber() {
            start(|| {
                yield_now();
            })
            .unwrap();
        }

        #[test]
        fn to_other_fiber() {
            start(|| {
                let changed = Rc::new(RefCell::new(false));

                spawn({
                    let changed = changed.clone();
                    move || *changed.borrow_mut() = true
                });

                assert!(!*changed.borrow());
                yield_now();

                assert!(*changed.borrow());
            })
            .unwrap();
        }
    }
}
