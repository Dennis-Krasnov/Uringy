//! Single-use notification to wake up an asynchronous task.
//!
//! You interact with oneshot notify through a pair of [`Notifier`] and [`Waiter`] handles that are created by the [`oneshot_notify`] function.
//! The [`Waiter`] implements [`Future`] which becomes ready once [`Notifier::notify()`] is called.
//!
//! Note that oneshot notify is a low level synchronization primitive which is used to implement higher-level concurrency patterns.
//! Your needs may be better met by [`crate::sync::notify`] for a multi-use notify and [`crate::sync::channel`] for passing messages between tasks.
//!
//! By design, the [`Notifier`] and [`Waiter`] don't provide additional methods that expose whether the other handle has been used or dropped.
//! This encourages loosely coupled tasks, leading to a better design.
//! If coupling between tasks is required, explicitly share state.
//!
//! # Examples
//!
//! ### Await Before Notify
//! Executing `waiter.await` before `notifier.notify()` suspends the current task.
//! The task will resume at some point after another task calls `notifier.notify()`.
//! ```
//! use uringy::runtime;
//! use uringy::sync::oneshot_notify::oneshot_notify;
//!
//! // run async block to completion
//! runtime::block_on(async {                              // Execution order:
//!     let (notifier, waiter) = oneshot_notify();         // 1
//!
//!     // create another task
//!     runtime::spawn(async move {                        // 2
//!         notifier.notify();                             // 4
//!         println!("sent notification");                 // 5                 
//!     });
//!
//!     waiter.await;                                      // 3
//!     println!("received notification");                 // 6
//! });
//! ```
//!
//! ### Await After Notify
//! Executing `waiter.await` after `notifier.notify()` doesn't suspend the current task.
//! ```
//! use uringy::runtime;
//! use uringy::sync::oneshot_notify::oneshot_notify;
//!
//! // run async block to completion
//! runtime::block_on(async {                              // Execution order:
//!     let (notifier, waiter) = oneshot_notify();         // 1
//!
//!     notifier.notify();                                 // 2
//!     println!("sent notification");                     // 3
//!
//!     waiter.await;                                      // 4
//!     println!("received notification");                 // 5
//! });
//! ```
//!
//! ### Unused Waiter
//! Dropping the waiter makes the `notifier.notify()` pointless.
//! ```
//! use uringy::runtime;
//! use uringy::sync::oneshot_notify::oneshot_notify;
//!
//! // run async block to completion
//! runtime::block_on(async {                              // Execution order:
//!     let (notifier, waiter) = oneshot_notify();         // 1
//!     drop(waiter);                                      // 2
//!
//!     notifier.notify();                                 // 3
//!     println!("sent notification to deaf ears");        // 4
//! });
//! ```
//!
//! ### Unused Notifier
//! Creating a cyclic dependency of waiters will cause a deadlock.
//! It's even possible to deadlock within a single task.
//! ```no_run
//! use uringy::runtime;
//! use uringy::sync::oneshot_notify::oneshot_notify;
//!
//! // run async block to completion
//! runtime::block_on(async {                              // Execution order:
//!     let (notifier, waiter) = oneshot_notify();         // 1
//!     drop(notifier);                                    // 2
//!
//!     waiter.await;                                      // 3
//!     unreachable!("Your task will hang forever!");
//! });
//! ```

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use NotifyState::*;

/// Creates a pair of [`Notifier`] and [`Waiter`] handles representing a single oneshot notification.
///
/// The [`Notifier`] handle is used to notify the `Waiter`.
/// The [`Waiter`] handle is used to await the notification.
///
/// Each handle can be used on separate tasks.
///
/// # Examples
/// ```
/// let (notifier, waiter) = uringy::sync::oneshot_notify::oneshot_notify();
/// ```
pub fn oneshot_notify() -> (Notifier, Waiter) {
    let state = Rc::new(RefCell::new(NotifyState::NothingHappened));

    let notifier = Notifier(state.clone());
    let waiter = Waiter(state);

    (notifier, waiter)
}

/// Internal state machine for a single oneshot notification.
#[derive(Debug)]
enum NotifyState {
    /// Initial idle state.
    NothingHappened,

    /// Pending notification.
    Notified,

    /// Waiter's task is waiting to be awoken.
    ///
    /// Option is necessary to take ownership of the [`Waker`] from a [`&mut Notify::Waiting`].
    /// The option value is always [`Option::Some`].
    Waiting(Option<Waker>),
}

/// Handle to notify the waiter.
#[derive(Debug)]
pub struct Notifier(Rc<RefCell<NotifyState>>);

impl Notifier {
    /// Notify the [`Waiter`].
    ///
    /// Consumes the [`Notifier`] since this is a single-use notification.
    pub fn notify(self) {
        let state = &mut *self.0.borrow_mut();

        match state {
            NothingHappened => {
                *state = Notified;
            }
            Notified => unreachable!(),
            Waiting(waker) => {
                let waker = waker.take().unwrap();
                *state = Notified;
                waker.wake();
            }
        }
    }
}

/// Awaitable handle for the notification.
#[derive(Debug)]
pub struct Waiter(Rc<RefCell<NotifyState>>);

impl Future for Waiter {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let state = &mut *self.0.borrow_mut();

        match state {
            NothingHappened | Waiting(_) => {
                *state = Waiting(Some(context.waker().clone()));
                Poll::Pending
            }
            Notified => Poll::Ready(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils;
    use impls::impls;
    use std::fmt::Debug;

    mod waiter {
        use super::*;

        #[test]
        fn pending_initially() {
            let (_notifier, mut waiter) = oneshot_notify();

            assert!(utils::poll(&mut waiter).is_pending());
        }

        #[test]
        fn ready_after_notify() {
            let (notifier, mut waiter) = oneshot_notify();

            notifier.notify();

            assert!(utils::poll(&mut waiter).is_ready());
        }

        #[test]
        fn pending_after_notifier_drop() {
            let (notifier, mut waiter) = oneshot_notify();

            // Waiter is unaware of this
            drop(notifier);

            assert!(utils::poll(&mut waiter).is_pending());
        }

        #[test]
        fn trait_implementations() {
            assert!(impls!(Waiter: Debug & !Send & !Sync));
        }
    }

    mod notifier {
        use super::*;

        #[test]
        fn unaware_of_dropped_waiter() {
            let (notifier, waiter) = oneshot_notify();

            drop(waiter);

            // Shouldn't panic
            notifier.notify();
        }

        #[test]
        fn trait_implementations() {
            assert!(impls!(Notifier: Debug & !Send & !Sync));
        }
    }
}
