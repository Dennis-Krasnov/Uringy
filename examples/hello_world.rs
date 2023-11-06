use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};

#[uringy::start]
fn main() {
    let mut stdout = unsafe { uringy::fs::File::from_raw_fd(std::io::stdout().as_raw_fd()) };

    stdout.write_all(b"hello world").unwrap();
}
