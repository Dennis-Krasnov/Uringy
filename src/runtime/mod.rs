//! Bridge between sync/async, green thread creation.

use std::future::Future;

pub(crate) mod io_uring;

mod event_loop;
mod task;

/// ...
pub fn block_on<OUT>(future: impl Future<Output = OUT> + 'static) -> OUT {
    event_loop::block_on(future, &event_loop::Config::default())
}

pub use event_loop::spawn;
pub use task::JoinHandle;

pub(crate) use event_loop::syscall;
