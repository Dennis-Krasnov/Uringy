use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// ...
pub fn poll<T>(future: &mut impl Future<Output = T>) -> Poll<T> {
    // Boilerplate for polling futures
    let waker = noop_waker::noop_waker();
    let mut context = Context::from_waker(&waker);

    // Pin to stack
    let future = unsafe { Pin::new_unchecked(future) };

    future.poll(&mut context)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Yield {
        yielded: bool,
    }

    impl Future for Yield {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
            if self.yielded {
                return Poll::Ready(());
            }

            self.yielded = true;
            context.waker().wake_by_ref();
            Poll::Pending
        }
    }

    #[test]
    fn poll_drives_future() {
        let mut future = Yield { yielded: false };
        assert_eq!(poll(&mut future), Poll::Pending);
        assert_eq!(poll(&mut future), Poll::Ready(()));
    }
}
