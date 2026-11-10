use std::str::FromStr;

use kernel::ids::{
    AdminId, ApprovalId, AuditId, ChallengeId, ChangeId, CredentialId, PreviewId, PublicationId,
    RevisionId, SessionId, VersionId,
};
use uuid::Uuid;

#[test]
fn uuid_v7_ids_reject_wrong_version() {
    let valid_v7 =
        Uuid::parse_str("018f4b2c-9d30-7a11-8b22-3c4d5e6f7081").expect("valid v7 fixture");
    let invalid_nil = Uuid::nil();
    let invalid_v4 =
        Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("valid v4 fixture");
    let invalid_max = Uuid::max();

    macro_rules! assert_v7_wrapper {
        ($ty:ty) => {{
            let accepted = <$ty>::try_from(valid_v7).expect("v7 UUID must be accepted");
            assert_eq!(accepted.as_uuid(), valid_v7);
            assert_eq!(Uuid::from(accepted), valid_v7);
            assert_eq!(accepted.to_string(), valid_v7.to_string());

            let parsed = <$ty>::from_str(&valid_v7.to_string()).expect("v7 string must parse");
            assert_eq!(parsed, accepted);

            let serialized = serde_json::to_string(&accepted).expect("v7 wrapper serializes");
            let deserialized: $ty =
                serde_json::from_str(&serialized).expect("v7 JSON deserializes");
            assert_eq!(deserialized, accepted);

            for bad in [invalid_nil, invalid_v4, invalid_max] {
                assert!(
                    <$ty>::try_from(bad).is_err(),
                    "non-v7 UUID {bad} must be rejected by {}",
                    stringify!($ty)
                );
                assert!(
                    <$ty>::from_str(&bad.to_string()).is_err(),
                    "non-v7 string {bad} must be rejected by {}",
                    stringify!($ty)
                );
                let bad_json = format!("\"{bad}\"");
                assert!(
                    serde_json::from_str::<$ty>(&bad_json).is_err(),
                    "non-v7 JSON {bad} must be rejected by {}",
                    stringify!($ty)
                );
            }
        }};
    }

    assert_v7_wrapper!(AdminId);
    assert_v7_wrapper!(CredentialId);
    assert_v7_wrapper!(SessionId);
    assert_v7_wrapper!(ChallengeId);
    assert_v7_wrapper!(ChangeId);
    assert_v7_wrapper!(RevisionId);
    assert_v7_wrapper!(PreviewId);
    assert_v7_wrapper!(ApprovalId);
    assert_v7_wrapper!(PublicationId);
    assert_v7_wrapper!(VersionId);
    assert_v7_wrapper!(AuditId);
}
