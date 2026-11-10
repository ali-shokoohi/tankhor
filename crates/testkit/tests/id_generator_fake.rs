use kernel::error::{ErrorCategory, ErrorCode};
use kernel::ports::id_generator::IdGenerator;
use testkit::contracts::id_generator::id_generator_contract;
use testkit::fakes::id_generator::SequenceIdGenerator;

#[tokio::test]
async fn id_generator_fake_passes_contract_and_is_deterministic() {
    id_generator_contract(&SequenceIdGenerator::new(1)).await;

    let first_run = SequenceIdGenerator::new(42);
    let second_run = SequenceIdGenerator::new(42);

    let a1 = first_run.next_admin_id().await.expect("a1");
    let a2 = first_run.next_audit_id().await.expect("a2");
    let b1 = second_run.next_admin_id().await.expect("b1");
    let b2 = second_run.next_audit_id().await.expect("b2");

    assert_eq!(
        a1, b1,
        "same seed must produce identical deterministic first ID"
    );
    assert_eq!(
        a2, b2,
        "same seed must produce identical deterministic second ID"
    );
    assert!(
        a1.as_uuid() < a2.as_uuid(),
        "sequential IDs must be strictly ordered"
    );

    let bounded = SequenceIdGenerator::with_limit(100, 2);
    let _ = bounded.next_change_id().await.expect("1st bounded ID");
    let _ = bounded.next_revision_id().await.expect("2nd bounded ID");
    let err = bounded
        .next_preview_id()
        .await
        .expect_err("exhausted deterministic sequence must return sanitized error");
    assert_eq!(err.category(), ErrorCategory::Unavailable);
    assert_eq!(err.code(), ErrorCode::SourceExhausted);
}
