//! Timeouts and delays.

use crate::runtime;
use std::time::Duration;

/// Waits until duration has elapsed.
pub async fn sleep(duration: Duration) {
    let result = runtime::syscall(
        io_uring::opcode::Timeout::new(
            &io_uring::types::Timespec::new()
                .sec(duration.as_secs())
                .nsec(duration.subsec_nanos()),
        )
        .build(),
    )
    .await;

    // Timeout got completed through expiration of the timer
    assert_eq!(result.unwrap_err().raw_os_error().unwrap(), libc::ETIME);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime;
    use std::time::Instant;

    mod sleep {
        use super::*;

        #[test]
        fn returns_immediately_with_zero() {
            // Problematic for timerfd-based implementations

            runtime::block_on(async {
                // Given
                let before = Instant::now();

                // When
                sleep(Duration::from_millis(0)).await;

                // Then
                assert!(before.elapsed() <= Duration::from_millis(5));
            });
        }

        #[test]
        fn passes_time() {
            runtime::block_on(async {
                // Given
                let before = Instant::now();

                // When
                sleep(Duration::from_millis(5)).await;

                // Then
                assert!(before.elapsed() >= Duration::from_millis(5));
            });
        }
    }
}
