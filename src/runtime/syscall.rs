//! Abstraction over cancellable non-blocking syscalls.
//!
//! Provides an implementation for every OS.

#[cfg(not(target_os = "linux"))]
compile_error!("Uringy only supports Linux");

#[cfg(target_os = "linux")]
pub(super) struct Interface {
    io_uring: io_uring::IoUring,
}

#[cfg(target_os = "linux")]
const ASYNC_CANCELLATION_USER_DATA: u64 = u64::MAX;

#[cfg(target_os = "linux")]
impl Interface {
    // TODO: optionally reuse kernel workers
    pub(super) fn new() -> Self {
        let mut builder = io_uring::IoUring::builder();
        builder.setup_clamp(); // won't panic if IORING_MAX_ENTRIES is too large
        let io_uring = builder.build(1024).unwrap();
        Interface { io_uring }
    }

    /// ...
    pub(super) fn wait_for_completed(&mut self) {
        self.io_uring.submit_and_wait(1).unwrap();
        // TODO: retry on EINTR (interrupted)
    }

    /// ...
    /// TODO: give this a closure?
    pub(super) fn process_completed(&mut self) -> impl Iterator<Item = (Id, i32)> {
        let mut results = vec![]; // TODO: return iterator (to avoid allocating) that mutably borrows io_uring by holding cq

        for cqe in self.io_uring.completion() {
            if cqe.user_data() == ASYNC_CANCELLATION_USER_DATA {
                continue;
            }

            let syscall_id = Id(cqe.user_data());

            // TODO: also process flags in match:
            // Storing the selected buffer ID, if one was selected. See BUFFER_SELECT for more info.
            // whether oneshot accepts needs to resubscribe (convert to yet another io::error)

            results.push((syscall_id, cqe.result()));
        }

        results.into_iter()
    }

    /// ...
    // TODO: make my own sqe struct (exposed to whole crate)
    pub(super) fn issue(&mut self, id: Id, sqe: io_uring::squeue::Entry) {
        let sqe = sqe.user_data(id.0);

        let mut sq = self.io_uring.submission();
        while sq.is_full() {
            drop(sq); // avoid borrowing io_uring more than once
                      // TODO: process CQs as well (same syscall)
            dbg!(self.io_uring.submit().unwrap()); // TODO: remove debug after ensuring this works
            sq = self.io_uring.submission();
        }
        unsafe { sq.push(&sqe).unwrap() }; // safety: submission queue isn't full
    }

    /// ...
    pub(super) fn cancel(&mut self, target: Id) {
        let sqe = io_uring::opcode::AsyncCancel::new(target.0).build();
        self.issue(Id(ASYNC_CANCELLATION_USER_DATA), sqe);
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(super) struct Id(pub(super) u64);
