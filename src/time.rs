use crate::runtime;
use std::time::Duration;

/// ...
pub fn sleep(duration: Duration) {
    let timespec = io_uring::types::Timespec::new()
        .sec(duration.as_secs())
        .nsec(duration.subsec_nanos());

    let sqe = io_uring::opcode::Timeout::new(&timespec).build();
    let result = runtime::syscall(sqe);
    assert_eq!(result.unwrap_err().raw_os_error().unwrap(), libc::ETIME);
}

// TODO: -> io::Result<()>to denote cancelled
// let error = result.unwrap_err();
// match error.raw_os_error().unwrap() {
//     libc::ETIME => Ok(()),
//     libc::ECANCELED => Err(error),
//     _ => unreachable!(),
// }

// TODO: timeout (requires cancellation and contained tasks)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime;
    use std::time::Instant;

    mod sleep {
        use super::*;

        #[test]
        fn doesnt_hang() {
            runtime::start(|| {
                let before = Instant::now();

                sleep(Duration::from_millis(0));

                assert!(before.elapsed() <= Duration::from_millis(5));
            });
        }

        #[test]
        fn passes_time() {
            runtime::start(|| {
                let before = Instant::now();

                sleep(Duration::from_millis(5));

                assert!(before.elapsed() >= Duration::from_millis(5));
            });
        }
    }
}
