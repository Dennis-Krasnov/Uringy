use std::time::Duration;

use crate::{runtime, Error};

/// Puts the current fiber to sleep for at least [duration].
pub fn sleep(duration: Duration) -> crate::CancellableResult<()> {
    let timespec = io_uring::types::Timespec::new()
        .sec(duration.as_secs())
        .nsec(duration.subsec_nanos());

    let sqe = io_uring::opcode::Timeout::new(&timespec).build();
    let result = runtime::syscall(sqe);

    match result {
        Ok(_) => unreachable!(),
        Err(error) => match error {
            Error::Original(e) => assert_eq!(e.raw_os_error().unwrap(), libc::ETIME),
            Error::Cancelled => return Err(Error::Cancelled),
        },
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use runtime::start;

    use super::*;

    mod sleep {
        use super::*;

        #[test]
        fn doesnt_hang_when_sleeping_zero() {
            start(|| {
                let before = Instant::now();

                sleep(Duration::from_millis(0)).unwrap();

                assert!(before.elapsed() < Duration::from_millis(5));
            })
            .unwrap();
        }

        #[test]
        fn passes_time() {
            start(|| {
                let before = Instant::now();

                sleep(Duration::from_millis(5)).unwrap();

                assert!(before.elapsed() > Duration::from_millis(5));
            })
            .unwrap();
        }
    }
}
