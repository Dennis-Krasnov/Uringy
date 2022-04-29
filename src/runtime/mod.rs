//! Bridge between sync/async and green threads.
// //! Runtime initialization and task spawning.

use std::future::Future;

mod event_loop;
mod task;

/// ...
///
/// near-identical docs with block_on in event loop module.
///
/// shortcut
pub fn block_on<OUT>(future: impl Future<Output = OUT>) -> OUT {
    event_loop::block_on(future, &event_loop::Config::default())
}

/// TODO: delete this, only used for benchmarking.
pub async fn nop() {
    syscall(io_uring::opcode::Nop::new().build()).await.unwrap();
}

pub use event_loop::block_on as block_on_with_options;
pub use event_loop::spawn;
pub use event_loop::Config;
pub use task::JoinHandle;

pub(crate) use event_loop::syscall;
