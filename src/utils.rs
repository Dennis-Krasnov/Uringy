use std::ops::{Bound, Range, RangeBounds};

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// ...
pub fn poll<T>(future: &mut impl Future<Output = T>) -> Poll<T> {
    // Boilerplate for polling futures
    let waker = noop_waker::noop_waker();
    let mut context = Context::from_waker(&waker);

    // Pin to stack
    let future = unsafe { Pin::new_unchecked(future) };

    future.poll(&mut context)
}

// TODO: test ^^^

/// Standardizes an arbitrary range bound into a range (a..b).
///
/// # Panics
/// - If the range's end is greater than the start
/// - If max is greater than min
pub(crate) fn range_from_bounds(
    range_bounds: impl RangeBounds<usize>,
    min: usize,
    max: usize,
) -> Range<usize> {
    assert!(min <= max);

    let start = match range_bounds.start_bound() {
        Bound::Included(&i) => i,
        Bound::Excluded(&i) => i + 1,
        Bound::Unbounded => min,
    };

    let end = match range_bounds.end_bound() {
        Bound::Included(&i) => i + 1,
        Bound::Excluded(&i) => i,
        Bound::Unbounded => max,
    };

    assert!(start <= end);

    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        assert_eq!(range_from_bounds(.., 2, 2), 2..2);
    }

    #[test]
    fn unbounded() {
        assert_eq!(range_from_bounds(.., 2, 5), 2..5);
    }

    #[test]
    fn start_bounded() {
        assert_eq!(range_from_bounds(2.., 2, 5), 2..5);
        assert_eq!(range_from_bounds(3.., 2, 5), 3..5);
    }

    #[test]
    fn end_bounded() {
        assert_eq!(range_from_bounds(..5, 2, 5), 2..5);
        assert_eq!(range_from_bounds(..4, 2, 5), 2..4);
    }

    #[test]
    fn end_bounded_inclusive() {
        assert_eq!(range_from_bounds(..=4, 2, 5), 2..5);
        assert_eq!(range_from_bounds(..=3, 2, 5), 2..4);
    }

    #[test]
    #[should_panic]
    fn start_greater_than_end() {
        range_from_bounds(10..5, 2, 5);
    }

    #[test]
    #[should_panic]
    fn min_greater_than_max() {
        range_from_bounds(.., 5, 2);
    }
}
