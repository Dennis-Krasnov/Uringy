#![allow(unused_imports)]
#![allow(dead_code)]

use benchmark::notify::*;
use iai::{black_box, main};

fn bench_create_destroy_uringy() {
    black_box(create_destroy_uringy());
}

fn bench_create_destroy_tokio() {
    black_box(create_destroy_tokio());
}

fn bench_notify_before_wait_uringy() {
    black_box(notify_before_wait_uringy());
}

fn bench_notify_before_wait_tokio() {
    black_box(notify_before_wait_tokio());
}

fn bench_notify_after_wait_uringy() {
    black_box(notify_after_wait_uringy());
}

fn bench_notify_after_wait_tokio() {
    black_box(notify_after_wait_tokio());
}

fn bench_wait_before_notify_uringy() {
    black_box(wait_before_notify_uringy());
}

fn bench_wait_before_notify_tokio() {
    black_box(wait_before_notify_tokio());
}

fn bench_wait_after_notify_uringy() {
    black_box(wait_after_notify_uringy());
}

fn bench_wait_after_notify_tokio() {
    black_box(wait_after_notify_tokio());
}

#[cfg(feature = "enable_iai")]
main!(
    bench_create_destroy_uringy,
    bench_create_destroy_tokio,
    bench_notify_before_wait_uringy,
    bench_notify_before_wait_tokio,
    bench_notify_after_wait_uringy,
    bench_notify_after_wait_tokio,
    bench_wait_before_notify_uringy,
    bench_wait_before_notify_tokio,
    bench_wait_after_notify_uringy,
    bench_wait_after_notify_tokio,
);

#[cfg(not(feature = "enable_iai"))]
fn main() {}
