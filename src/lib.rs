#![warn(missing_docs)]

use std::future::Future;


use crate::event_loop::EventLoop;

mod event_loop;

/// ...
///
/// # Examples
///
/// ### Run on current thread
/// ```
/// uringy::block_on(async {});
/// ```
///
/// ### Run on new thread
/// ```
/// std::thread::spawn(|| {
///     uringy::block_on(async {});
/// }).join().unwrap();
/// ```
pub fn block_on(future: impl Future<Output=()> + 'static) {
    let mut event_loop = EventLoop::new();
    event_loop.spawn(future);
    event_loop.run();
}

/// ... uses thread local instance of event loop
///
/// # Panics
/// If not run on thread with running event loop...
pub fn spawn<F>(_future: F) where F: Future<Output=()> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {
        block_on(async {
            println!("HI");
            let (s, r) = async_channel::bounded(1);
            s.send(1).await.unwrap();
            assert_eq!(Ok(1), r.recv().await);
        });
    }

    #[test]
    fn simple2() {
        println!("HI");

        let (s, r) = async_channel::unbounded();

        let mut event_loop = EventLoop::new();

        event_loop.spawn(async move {
            for n in 0..10 {
                s.send(n).await.unwrap();
            }
        });

        event_loop.spawn(async move {
            while let Ok(n) = r.recv().await {
                println!("it's {}", n);
            }
        });

        event_loop.run();
    }
}
