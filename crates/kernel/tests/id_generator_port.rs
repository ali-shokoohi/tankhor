use kernel::error::{DomainError, ErrorCode};
use kernel::ids::{
    AdminId, ApprovalId, AuditId, ChallengeId, ChangeId, CredentialId, PreviewId, PublicationId,
    RevisionId, SessionId, VersionId,
};
use kernel::ports::id_generator::{IdGenerator, MockIdGenerator};
use uuid::Uuid;

fn sample_v7(counter: u16) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[..6].copy_from_slice(&0x0195_0000_0000u64.to_be_bytes()[2..]);
    bytes[6] = 0x70 | ((counter >> 8) as u8 & 0x0f);
    bytes[7] = (counter & 0xff) as u8;
    bytes[8] = 0x80;
    bytes[15] = (counter & 0xff) as u8;
    Uuid::from_bytes(bytes)
}

fn assert_automock_above_async_trait() {
    let trait_source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/ports/id_generator.rs"
    ))
    .expect("id_generator port file must exist");
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

async fn verify_exhausted_generator_error() {
    let mut exhausted = MockIdGenerator::new();
    exhausted
        .expect_next_admin_id()
        .times(1)
        .returning(|| Err(DomainError::unavailable(ErrorCode::SourceExhausted)));
    let err = exhausted
        .next_admin_id()
        .await
        .expect_err("exhausted generator returns domain error");
    assert_eq!(err.code(), ErrorCode::SourceExhausted);
}

#[tokio::test]
async fn id_generator_port_is_async_and_automockable() {
    let expected_admin = AdminId::try_new(sample_v7(1)).expect("valid v7");
    let expected_credential = CredentialId::try_new(sample_v7(2)).expect("valid v7");
    let expected_session = SessionId::try_new(sample_v7(3)).expect("valid v7");
    let expected_challenge = ChallengeId::try_new(sample_v7(4)).expect("valid v7");
    let expected_change = ChangeId::try_new(sample_v7(5)).expect("valid v7");
    let expected_revision = RevisionId::try_new(sample_v7(6)).expect("valid v7");
    let expected_preview = PreviewId::try_new(sample_v7(7)).expect("valid v7");
    let expected_approval = ApprovalId::try_new(sample_v7(8)).expect("valid v7");
    let expected_publication = PublicationId::try_new(sample_v7(9)).expect("valid v7");
    let expected_version = VersionId::try_new(sample_v7(10)).expect("valid v7");
    let expected_audit = AuditId::try_new(sample_v7(11)).expect("valid v7");

    let mut mock = MockIdGenerator::new();
    mock.expect_next_admin_id()
        .times(1)
        .returning(move || Ok(expected_admin));
    mock.expect_next_credential_id()
        .times(1)
        .returning(move || Ok(expected_credential));
    mock.expect_next_session_id()
        .times(1)
        .returning(move || Ok(expected_session));
    mock.expect_next_challenge_id()
        .times(1)
        .returning(move || Ok(expected_challenge));
    mock.expect_next_change_id()
        .times(1)
        .returning(move || Ok(expected_change));
    mock.expect_next_revision_id()
        .times(1)
        .returning(move || Ok(expected_revision));
    mock.expect_next_preview_id()
        .times(1)
        .returning(move || Ok(expected_preview));
    mock.expect_next_approval_id()
        .times(1)
        .returning(move || Ok(expected_approval));
    mock.expect_next_publication_id()
        .times(1)
        .returning(move || Ok(expected_publication));
    mock.expect_next_version_id()
        .times(1)
        .returning(move || Ok(expected_version));
    mock.expect_next_audit_id()
        .times(1)
        .returning(move || Ok(expected_audit));

    let g: &dyn IdGenerator = &mock;
    assert_eq!(g.next_admin_id().await.expect("admin"), expected_admin);
    assert_eq!(
        g.next_credential_id().await.expect("credential"),
        expected_credential
    );
    assert_eq!(
        g.next_session_id().await.expect("session"),
        expected_session
    );
    assert_eq!(
        g.next_challenge_id().await.expect("challenge"),
        expected_challenge
    );
    assert_eq!(g.next_change_id().await.expect("change"), expected_change);
    assert_eq!(
        g.next_revision_id().await.expect("revision"),
        expected_revision
    );
    assert_eq!(
        g.next_preview_id().await.expect("preview"),
        expected_preview
    );
    assert_eq!(
        g.next_approval_id().await.expect("approval"),
        expected_approval
    );
    assert_eq!(
        g.next_publication_id().await.expect("publication"),
        expected_publication
    );
    assert_eq!(
        g.next_version_id().await.expect("version"),
        expected_version
    );
    assert_eq!(g.next_audit_id().await.expect("audit"), expected_audit);

    verify_exhausted_generator_error().await;
    assert_automock_above_async_trait();
}
