//! Non-blocking syscall interface that supports cancellation.

use std::io;
#[cfg(not(target_os = "linux"))]
compile_error!("Uringy only supports Linux");

#[cfg(target_os = "linux")]
pub(super) struct Uring {
    io_uring: io_uring::IoUring,
}

// #[cfg(target_os = "linux")]
// const ASYNC_CANCELLATION: UserData = UserData(u64::MAX);

#[cfg(target_os = "linux")]
impl Uring {
    /// ...
    pub(super) fn new() -> Self {
        let mut builder = io_uring::IoUring::builder();
        builder.setup_clamp(); // won't panic if IORING_MAX_ENTRIES is too large
        let io_uring = builder.build(1024).unwrap();
        Uring { io_uring }
    }

    /// ...
    pub(super) fn wait_for_completed_syscall(&mut self) {
        self.io_uring.submit_and_wait(1).unwrap();
        // TODO: retry on EINTR
    }

    /// ...
    pub(super) fn process_cq(&mut self) -> Vec<(UserData, io::Result<u32>)> {
        let mut results = vec![]; // TODO: return iterator (to avoid allocating) that mutably borrows io_uring by holding cq

        for cqe in self.io_uring.completion() {
            // if cqe.user_data() == ASYNC_CANCELLATION.0 {
            //     continue;
            // }

            let user_data = UserData(cqe.user_data());

            let result = if cqe.result() >= 0 {
                Ok(cqe.result() as u32)
            } else {
                Err(io::Error::from_raw_os_error(-cqe.result()))
            };

            // TODO: also process flags in match:
            // Storing the selected buffer ID, if one was selected. See BUFFER_SELECT for more info.
            // whether oneshot accepts needs to resubscribe (convert to yet another io::error)

            results.push((user_data, result));
        }

        results
    }

    // /// ...
    // pub(super) fn cancel_syscall(&mut self, user_data: UserData) {
    //     let sqe = io_uring::opcode::AsyncCancel::new(user_data.0).build();
    //     self.issue_syscall(ASYNC_CANCELLATION, sqe);
    // }

    /// ...
    // TODO: make my own sqe struct (exposed to whole crate)
    pub(super) fn issue_syscall(&mut self, user_data: UserData, sqe: io_uring::squeue::Entry) {
        let sqe = sqe.user_data(user_data.0);

        let mut sq = self.io_uring.submission();
        while sq.is_full() {
            drop(sq); // avoid borrowing io_uring more than once
            dbg!(self.io_uring.submit().unwrap()); // TODO: remove debug after ensuring this works
            sq = self.io_uring.submission();
        }
        unsafe { sq.push(&sqe).unwrap() }; // safety: submission queue isn't full
    }
}

// TODO: compatibility mode (optional mio dependency)
// #[cfg(any(target_os = "macos", target_os = "windows"))]
// pub(super) struct Uring;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub(super) struct UserData(pub(super) u64);
