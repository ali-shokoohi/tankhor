use kernel::error::{DomainError, ErrorCode};
use kernel::ports::clock::{Clock, MockClock};
use time::OffsetDateTime;

#[tokio::test]
async fn clock_port_is_async_and_automockable() {
    let expected = OffsetDateTime::from_unix_timestamp(1_762_000_000).expect("valid timestamp");
    let mut mock = MockClock::new();
    mock.expect_now().times(1).returning(move || Ok(expected));

    let clock: &dyn Clock = &mock;
    let observed = clock.now().await.expect("mock clock returns UTC instant");
    assert_eq!(observed, expected);
    assert_eq!(observed.offset().whole_seconds(), 0);

    let mut failing = MockClock::new();
    failing
        .expect_now()
        .times(1)
        .returning(|| Err(DomainError::unavailable(ErrorCode::SourceExhausted)));

    let err = failing
        .now()
        .await
        .expect_err("clock failure maps to domain error");
    assert_eq!(err.code(), ErrorCode::SourceExhausted);

    let trait_source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ports/clock.rs"))
            .expect("clock port file must exist");
    let automock_pos = trait_source
        .find("cfg_attr(any(test, feature = \"mocks\"), mockall::automock)")
        .expect("automock attribute must be present");
    let async_trait_pos = trait_source
        .find("async_trait::async_trait")
        .expect("async_trait attribute must be present");
    assert!(
        automock_pos < async_trait_pos,
        "mockall::automock must appear before async_trait::async_trait"
    );
}
