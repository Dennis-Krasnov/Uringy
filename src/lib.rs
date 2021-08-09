use std::future::Future;
use waker_fn::waker_fn;
use std::task::{Context, Poll};
use std::pin::Pin;


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
pub fn block_on<F>(future: F) where F: Future<Output = ()> { //  + 'static
    let (parker, unparker) = parking::pair();

    let waker = waker_fn(move || {
        unparker.unpark();
    });

    let mut cx = Context::from_waker(&waker);

    let mut future = Box::pin(future);

    loop {
        match Pin::new(&mut future).poll(&mut cx) {
            Poll::Ready(_) => { break; }
            Poll::Pending => { parker.park(); }
        }
    }
}

/// ... uses thread local instance of event loop
///
/// # Panics
/// If not run on thread with running event loop...
pub fn spawn<F>(future: F) where F: Future<Output = ()> { // + 'static

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple() {

    }
}
