//! Thread local storage.... encapsulates.
//! easy switch to fast thread local / unsafe cell.

use std::cell::RefCell;

/// Cache padded to avoid potential performance hit due to false sharing.
#[repr(align(128))]
struct Runtime(RefCell<Option<super::RuntimeState>>);

#[cfg(not(feature = "fast_thread_local"))]
thread_local! {
    /// Each thread gets its own independent runtime.
    static RUNTIME: Runtime = Runtime(RefCell::new(None));
}

#[cfg(not(feature = "fast_thread_local"))]
pub(super) fn exclusive_runtime<T>(f: impl FnOnce() -> T) -> T {
    RUNTIME.with(|thread_local| {
        let mut cell = thread_local.0.borrow_mut();
        assert!(cell.is_none(), "...");
        *cell = Some(super::RuntimeState::new());
    });

    let output = f();

    RUNTIME.with(|thread_local| {
        let mut cell = thread_local.0.borrow_mut();
        *cell = None;
    });

    output
}

/// Borrow ...
#[cfg(not(feature = "fast_thread_local"))]
pub(super) fn runtime<T>(f: impl FnOnce(&mut super::RuntimeState) -> T) -> T {
    RUNTIME.with(|thread_local| {
        let mut cell = thread_local.0.borrow_mut();
        let runtime = cell.as_mut().expect("no runtime...");
        f(runtime)
    })
}

#[cfg(feature = "fast_thread_local")]
#[thread_local]
static RUNTIME: Runtime = Runtime(RefCell::new(None));

#[cfg(feature = "fast_thread_local")]
pub(super) fn exclusive_runtime<T>(f: impl FnOnce() -> T) -> T {
    {
        let mut cell = RUNTIME.0.borrow_mut();
        assert!(cell.is_none(), "...");
        *cell = Some(super::RuntimeState::new());
    }

    let output = f();

    let mut cell = RUNTIME.0.borrow_mut();
    *cell = None;

    output
}

/// Borrow ...
#[cfg(feature = "fast_thread_local")]
pub(super) fn runtime<T>(f: impl FnOnce(&mut super::RuntimeState) -> T) -> T {
    let mut cell = RUNTIME.0.borrow_mut();
    let runtime = cell.as_mut().expect("no runtime...");
    f(runtime)
}
