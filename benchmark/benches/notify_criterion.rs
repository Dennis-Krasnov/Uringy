use benchmark::notify::*;
use criterion::async_executor::AsyncExecutor;
use criterion::{criterion_group, criterion_main, Criterion};
use std::future::Future;

pub struct UringyExecutor;
impl AsyncExecutor for UringyExecutor {
    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        uringy::block_on(future)
    }
}

pub fn bench_create_destroy(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/create_destroy");
    group.bench_function("uringy", |b| {
        b.to_async(UringyExecutor)
            .iter(|| async { create_destroy_uringy() })
    });
    group.bench_function("tokio", |b| b.iter(|| create_destroy_tokio()));
    group.finish();
}

pub fn bench_notify_before_wait(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/notify_before_wait");
    group.bench_function("uringy", |b| {
        b.to_async(UringyExecutor)
            .iter(|| async { notify_before_wait_uringy() })
    });
    group.bench_function("tokio", |b| b.iter(|| notify_before_wait_tokio()));
    group.finish();
}

pub fn bench_notify_after_wait(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/notify_after_wait");
    group.bench_function("uringy", |b| {
        b.to_async(UringyExecutor)
            .iter(|| async { notify_after_wait_uringy() })
    });
    group.bench_function("tokio", |b| b.iter(|| notify_after_wait_tokio()));
    group.finish();
}

pub fn bench_wait_before_notify(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/wait_before_notify");
    group.bench_function("uringy", |b| {
        b.to_async(UringyExecutor)
            .iter(|| async { wait_before_notify_uringy() })
    });
    group.bench_function("tokio", |b| b.iter(|| wait_before_notify_tokio()));
    group.finish();
}

pub fn bench_wait_after_notify(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/wait_after_notify");
    group.bench_function("uringy", |b| {
        b.to_async(UringyExecutor)
            .iter(|| async { wait_after_notify_uringy() })
    });
    group.bench_function("tokio", |b| b.iter(|| wait_after_notify_tokio()));
    group.finish();
}

// hypothesis: not allocating each time may be beneficial for uringy version
pub fn bench_many_wait_after_notify(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/wait_after_notify");
    group.bench_function("uringy", |b| {
        b.to_async(UringyExecutor).iter(|| async {
            for _ in 0..1_000_000 {
                wait_after_notify_uringy()
            }
        })
    });
    group.bench_function("tokio", |b| {
        b.iter(|| {
            for _ in 0..1_000_000 {
                wait_after_notify_tokio()
            }
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_create_destroy,
    bench_notify_before_wait,
    bench_notify_after_wait,
    bench_wait_before_notify,
    bench_wait_after_notify,
    bench_many_wait_after_notify,
);
criterion_main!(benches);
