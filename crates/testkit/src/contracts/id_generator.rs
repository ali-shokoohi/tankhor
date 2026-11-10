//! Shared infrastructure-neutral contract suite for [`IdGenerator`].
//!
//! Runs unchanged against both [`SequenceIdGenerator`](crate::fakes::id_generator::SequenceIdGenerator)
//! and the real `UUIDv7` generator adapter without requiring identical random outputs.

use std::collections::HashSet;

use kernel::ports::id_generator::IdGenerator;
use uuid::Version;

/// Execute the shared [`IdGenerator`] contract against any adapter implementation.
///
/// # Panics
///
/// Panics if any generated identifier is not RFC 9562 `UUIDv7` or if distinct allocations collide.
pub async fn id_generator_contract(generator: &impl IdGenerator) {
    let mut seen = HashSet::new();

    let admin_a = generator.next_admin_id().await.expect("admin_id a");
    let admin_b = generator.next_admin_id().await.expect("admin_id b");
    let credential = generator.next_credential_id().await.expect("credential_id");
    let session = generator.next_session_id().await.expect("session_id");
    let challenge = generator.next_challenge_id().await.expect("challenge_id");
    let change = generator.next_change_id().await.expect("change_id");
    let revision = generator.next_revision_id().await.expect("revision_id");
    let preview = generator.next_preview_id().await.expect("preview_id");
    let approval = generator.next_approval_id().await.expect("approval_id");
    let publication = generator
        .next_publication_id()
        .await
        .expect("publication_id");
    let version = generator.next_version_id().await.expect("version_id");
    let audit = generator.next_audit_id().await.expect("audit_id");

    let raw_ids = [
        admin_a.as_uuid(),
        admin_b.as_uuid(),
        credential.as_uuid(),
        session.as_uuid(),
        challenge.as_uuid(),
        change.as_uuid(),
        revision.as_uuid(),
        preview.as_uuid(),
        approval.as_uuid(),
        publication.as_uuid(),
        version.as_uuid(),
        audit.as_uuid(),
    ];

    for raw in raw_ids {
        assert_eq!(
            raw.get_version(),
            Some(Version::SortRand),
            "every generated identifier must be UUID version 7"
        );
        assert_eq!(
            raw.get_variant(),
            uuid::Variant::RFC4122,
            "every generated identifier must carry RFC 9562 variant bits"
        );
        assert!(
            seen.insert(raw),
            "every generated identifier allocation must be unique"
        );
    }
}
