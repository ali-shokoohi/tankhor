//! Shared infrastructure-neutral contract suite for [`Clock`].
//!
//! Runs unchanged against both [`FixedClock`](crate::fakes::clock::FixedClock) and the
//! real system clock adapter without assuming wall-clock monotonicity between calls.

use kernel::ports::clock::Clock;
use time::UtcOffset;

/// Execute the shared [`Clock`] contract against any adapter implementation.
///
/// # Panics
///
/// Panics if the clock returns an error on a healthy probe, returns a non-UTC offset,
/// or yields an out-of-range modern timestamp.
pub async fn clock_contract(clock: &impl Clock) {
    let first = clock
        .now()
        .await
        .expect("healthy Clock implementation must return a UTC instant");
    let second = clock
        .now()
        .await
        .expect("repeated Clock::now calls must succeed");

    assert_eq!(
        first.offset(),
        UtcOffset::UTC,
        "Clock::now must return an explicit UTC instant"
    );
    assert_eq!(
        second.offset(),
        UtcOffset::UTC,
        "Clock::now must preserve UTC offset across calls"
    );
    assert!(
        first.year() >= 2020 && second.year() >= 2020,
        "Clock::now must return plausible modern instants"
    );
}
