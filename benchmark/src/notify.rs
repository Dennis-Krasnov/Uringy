use std::future::Future;
use std::task::{Context, Poll};

pub fn create_destroy_uringy() -> (uringy::event_loop::Notifier, uringy::event_loop::Waiter) {
    uringy::notify()
}

pub fn create_destroy_tokio() {
    let notifier = tokio::sync::Notify::new();
    let _waiter = notifier.notified();
}

pub fn notify_before_wait_uringy() {
    let (notifier, _waiter) = uringy::notify();
    notifier.notify();
}

pub fn notify_before_wait_tokio() {
    let notifier = tokio::sync::Notify::new();
    let _waiter = notifier.notified();
    notifier.notify_one();
}

pub fn notify_after_wait_uringy() {
    let waker = noop_waker::noop_waker();
    let mut ctx = Context::from_waker(&waker);

    let (notifier, mut waiter) = uringy::notify();

    let waiter = unsafe { std::pin::Pin::new_unchecked(&mut waiter) };
    assert_eq!(waiter.poll(&mut ctx), Poll::Pending);

    notifier.notify();
}

pub fn notify_after_wait_tokio() {
    let waker = noop_waker::noop_waker();
    let mut ctx = Context::from_waker(&waker);

    let notifier = tokio::sync::Notify::new();
    let mut waiter = notifier.notified();

    let waiter = unsafe { std::pin::Pin::new_unchecked(&mut waiter) };
    assert_eq!(waiter.poll(&mut ctx), Poll::Pending);

    notifier.notify_one();
}

pub fn wait_before_notify_uringy() {
    let waker = noop_waker::noop_waker();
    let mut ctx = Context::from_waker(&waker);

    let (_notifier, mut waiter) = uringy::notify();

    let waiter = unsafe { std::pin::Pin::new_unchecked(&mut waiter) };
    assert_eq!(waiter.poll(&mut ctx), Poll::Pending);
}

pub fn wait_before_notify_tokio() {
    let waker = noop_waker::noop_waker();
    let mut ctx = Context::from_waker(&waker);

    let notifier = tokio::sync::Notify::new();
    let mut waiter = notifier.notified();

    let waiter = unsafe { std::pin::Pin::new_unchecked(&mut waiter) };
    assert_eq!(waiter.poll(&mut ctx), Poll::Pending);
}

pub fn wait_after_notify_uringy() {
    let waker = noop_waker::noop_waker();
    let mut ctx = Context::from_waker(&waker);

    let (notifier, mut waiter) = uringy::notify();

    notifier.notify();

    let waiter = unsafe { std::pin::Pin::new_unchecked(&mut waiter) };
    assert_eq!(waiter.poll(&mut ctx), Poll::Ready(()));
}

pub fn wait_after_notify_tokio() {
    let waker = noop_waker::noop_waker();
    let mut ctx = Context::from_waker(&waker);

    let notifier = tokio::sync::Notify::new();
    let mut waiter = notifier.notified();

    notifier.notify_one();

    let waiter = unsafe { std::pin::Pin::new_unchecked(&mut waiter) };
    assert_eq!(waiter.poll(&mut ctx), Poll::Ready(()));
}
