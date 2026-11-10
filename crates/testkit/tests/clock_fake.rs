use kernel::error::{ErrorCategory, ErrorCode};
use kernel::ports::clock::Clock;
use testkit::contracts::clock::clock_contract;
use testkit::fakes::clock::FixedClock;
use time::{Duration, OffsetDateTime};

#[tokio::test]
async fn clock_fake_passes_contract_and_controls_boundaries() {
    let base = OffsetDateTime::from_unix_timestamp(1_762_000_000).expect("valid UTC timestamp");
    clock_contract(&FixedClock::new(base)).await;

    let clock = FixedClock::new(base);
    assert_eq!(clock.now().await.expect("initial instant"), base);

    clock.advance(Duration::seconds(30));
    assert_eq!(
        clock.now().await.expect("advanced instant"),
        base + Duration::seconds(30)
    );

    let custom = base - Duration::minutes(5);
    clock.set(custom);
    assert_eq!(clock.now().await.expect("explicitly set instant"), custom);

    let non_utc = base.to_offset(time::UtcOffset::from_hms(3, 30, 0).expect("valid offset"));
    clock.set(non_utc);
    let normalized = clock.now().await.expect("normalized to UTC");
    assert_eq!(normalized.offset().whole_seconds(), 0);
    assert_eq!(normalized.unix_timestamp(), non_utc.unix_timestamp());

    let unavailable = FixedClock::unavailable();
    let err = unavailable
        .now()
        .await
        .expect_err("unavailable fake clock must fail with sanitized error");
    assert_eq!(err.category(), ErrorCategory::Unavailable);
    assert_eq!(err.code(), ErrorCode::SourceExhausted);
}
