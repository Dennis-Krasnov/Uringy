use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn uringy_create_notify() {
    black_box(uringy::sync::Notify::new());
}

pub fn tokio_create_notify() {
    black_box(tokio::sync::Notify::new());
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("uringy_create_notify", |b| {
        b.iter(|| uringy_create_notify())
    });
    c.bench_function("tokio_create_notify", |b| b.iter(|| tokio_create_notify()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
