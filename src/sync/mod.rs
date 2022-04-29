//! Asynchronous synchronization primitives.
//!
//! Concurrency is about dealing with a lot of things at the same time. (efficiency - better utilization of CPU - "working smarter")
//! Parallelism is about doing a lot of things at the same time. (nothing efficient about using multiple resources at once)
//!
//! I can see two major use cases for concurrency:
//! When performing I/O and you need to wait for some external event to occur
//! When you need to divide your attention and prevent one task from waiting too long
//!
//! when to use each primitive (flow chart)

// pub mod channel;
// pub mod notify;
pub mod oneshot_notify;
// pub mod semaphore;
