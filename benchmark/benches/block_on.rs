use criterion::{criterion_group, criterion_main, Criterion};

pub fn uringy_block_on() {
    uringy::block_on(async {});
}

pub fn tokio_block_on() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    rt.block_on(async {});
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("uringy_block_on", |b| b.iter(|| uringy_block_on()));

    c.bench_function("tokio_block_on", |b| b.iter(|| tokio_block_on()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
