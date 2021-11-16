use iai::{black_box, main};

fn uringy_create_notify() {
    black_box(uringy::sync::notify::notify());
}

fn tokio_create_notify() {
    black_box(tokio::sync::Notify::new());
}

main!(uringy_create_notify, tokio_create_notify);
