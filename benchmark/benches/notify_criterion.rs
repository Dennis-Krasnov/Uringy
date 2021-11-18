use benchmark::notify::*;
use criterion::{criterion_group, criterion_main, Criterion};

pub fn bench_create_destroy(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/create_destroy");
    group.bench_function("uringy", |b| b.iter(|| create_destroy_uringy()));
    group.bench_function("tokio", |b| b.iter(|| create_destroy_tokio()));
    group.finish();
}

pub fn bench_notify_before_wait(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/notify_before_wait");
    group.bench_function("uringy", |b| b.iter(|| notify_before_wait_uringy()));
    group.bench_function("tokio", |b| b.iter(|| notify_before_wait_tokio()));
    group.finish();
}

pub fn bench_notify_after_wait(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/notify_after_wait");
    group.bench_function("uringy", |b| b.iter(|| notify_after_wait_uringy()));
    group.bench_function("tokio", |b| b.iter(|| notify_after_wait_tokio()));
    group.finish();
}

pub fn bench_wait_before_notify(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/wait_before_notify");
    group.bench_function("uringy", |b| b.iter(|| wait_before_notify_uringy()));
    group.bench_function("tokio", |b| b.iter(|| wait_before_notify_tokio()));
    group.finish();
}

pub fn bench_wait_after_notify(c: &mut Criterion) {
    let mut group = c.benchmark_group("notify/wait_after_notify");
    group.bench_function("uringy", |b| b.iter(|| wait_after_notify_uringy()));
    group.bench_function("tokio", |b| b.iter(|| wait_after_notify_tokio()));
    group.finish();
}

criterion_group!(
    benches,
    bench_create_destroy,
    bench_notify_before_wait,
    bench_notify_after_wait,
    bench_wait_before_notify,
    bench_wait_after_notify,
);
criterion_main!(benches);
