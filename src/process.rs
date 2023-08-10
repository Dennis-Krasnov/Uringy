use crate::runtime;
use std::io;

pub fn print(s: &str) -> io::Result<()> {
    let stdout = io_uring::types::Fd(1);
    let sqe = io_uring::opcode::Write::new(stdout, s.as_ptr(), s.len() as u32).build();
    let result = runtime::syscall(sqe)?; // TODO: method on runtime?
    assert_eq!(result, s.len() as u32);

    Ok(())
}

pub fn eprint(s: &str) -> io::Result<()> {
    let stderr = io_uring::types::Fd(2);
    let sqe = io_uring::opcode::Write::new(stderr, s.as_ptr(), s.len() as u32).build();
    let result = runtime::syscall(sqe)?;
    assert_eq!(result, s.len() as u32);

    Ok(())
}
