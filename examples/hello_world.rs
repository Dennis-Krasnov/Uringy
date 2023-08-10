fn main() {
    uringy::runtime::start(|| {
        uringy::process::print("hello ").unwrap();
        uringy::process::print("world!\n").unwrap();
    });
}
