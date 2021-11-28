use criterion::async_executor::AsyncExecutor;
use criterion::{criterion_group, criterion_main, Criterion};
use std::future::Future;
use std::rc::Rc;
use uringy::spawn;

// TODO: move this to src/lib
pub struct UringyExecutor;
impl AsyncExecutor for UringyExecutor {
    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        uringy::block_on(future)
    }
}

pub fn bench_create_destroy(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/create_destroy");

    group.bench_function("uringy", |b| {
        b.iter(|| uringy::notify::notify());
    });

    group.bench_function("tokio", |b| {
        b.iter(|| {
            let notifier = tokio::sync::Notify::new();
            let _waiter = notifier.notified();
        });
    });

    group.finish();
}

pub fn bench_notify_before_wait(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/notify_before_wait");

    group.bench_function("baseline_overhead", |b| {
        b.to_async(UringyExecutor).iter(|| async { async {}.await });
    });

    group.bench_function("uringy", |b| {
        b.to_async(UringyExecutor).iter(|| async {
            let (notifier, waiter) = uringy::notify::notify();

            notifier.notify();

            waiter.await;
        });
    });

    group.bench_function("tokio", |b| {
        b.to_async(UringyExecutor).iter(|| async {
            let notifier = tokio::sync::Notify::new();
            let waiter = notifier.notified();

            notifier.notify_one();

            waiter.await;
        });
    });

    group.finish();
}

pub fn bench_notify_after_wait(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/notify_after_wait");

    group.bench_function("baseline_overhead", |b| {
        b.to_async(UringyExecutor).iter(|| async {
            spawn(async {}).detach();
            async {}.await
        });
    });

    group.bench_function("uringy", |b| {
        b.to_async(UringyExecutor).iter(|| async {
            let (notifier, waiter) = uringy::notify::notify();

            spawn(async move {
                notifier.notify();
            })
            .detach();

            waiter.await;
        });
    });

    group.bench_function("tokio", |b| {
        b.to_async(UringyExecutor).iter(|| async {
            let notifier = Rc::new(tokio::sync::Notify::new());
            let waiter = notifier.notified();

            let notifier_copy = notifier.clone();
            spawn(async move {
                notifier_copy.notify_one();
            })
            .detach();

            waiter.await;
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_create_destroy,
    bench_notify_before_wait,
    bench_notify_after_wait,
);
criterion_main!(benches);
