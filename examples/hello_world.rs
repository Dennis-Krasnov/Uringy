#[uringy::start]
fn main() {
    uringy::process::print("hello ").unwrap();
    uringy::process::print("world!\n").unwrap();
}
