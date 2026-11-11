//! T164: enrollment authorization and primary proof are jointly necessary, never access.
use admin::{
    application::enrollment::{BeginPasskeyEnrollment, BeginPasskeyEnrollmentInput},
    domain::{
        admin_state::{EnrollmentPurpose, SecurityEpoch},
        auth_state::{AuthPurpose, AuthStage, BindingDigest},
    },
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    audit_repository::FakeAuditRepository, clock::FixedClock, id_generator::SequenceIdGenerator,
    session_store::FakeSessionStore, store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/enrollment_fixture.rs"]
mod fixture;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "independent authorization and primary expiry cases in original begin driver"
)]
async fn passkey_enrollment_authorization_is_not_console_access() {
    for scenario in [
        "valid",
        "absent",
        "expired",
        "authorization_expired",
        "primary_expired",
        "browser",
        "session",
        "purpose",
        "stage",
        "target",
        "epoch",
        "reference",
        "audit",
    ] {
        let store = FakeStore::new();
        let uow = FakeUnitOfWork::new(store.clone());
        let admins = FakeAdminRepository::new(store.clone());
        let sessions = FakeSessionStore::new(store.clone());
        let audit = FakeAuditRepository::new(store.clone());
        let crypto = FakeAdminAuthenticator::controlled_fixture();
        let clock = FixedClock::new(fixture::now());
        let ids = SequenceIdGenerator::new(100);
        store.seed(|state| {
            let target = if scenario == "authorization_expired" {
                fixture::target_until(true, fixture::now() + time::Duration::seconds(30))
            } else {
                fixture::target(scenario != "absent")
            };
            state.admins.insert(target.id(), target);
            state.admin_pending.insert([5; 32], fixture::pending());
        });
        if scenario == "expired" {
            clock.advance(time::Duration::minutes(5));
        }
        if scenario == "authorization_expired" {
            clock.advance(time::Duration::seconds(30));
        }
        if scenario == "primary_expired" {
            clock.advance(time::Duration::MINUTE);
        }
        if scenario == "audit" {
            audit.fail_next_append();
        }
        let mut proof = fixture::check();
        match scenario {
            "browser" => proof.binding.browser = BindingDigest::new([99; 32]),
            "session" => {
                proof.binding.session = "019a53a0-0000-7000-8000-000000000099".parse().unwrap();
            }
            "purpose" => {
                proof.binding.purpose = AuthPurpose::Enrollment(EnrollmentPurpose::Recovery);
            }
            "stage" => proof.binding.stage = AuthStage::Passkey,
            "target" => {
                proof.binding.target = "019a53a0-0000-7000-8000-000000000099".parse().unwrap();
            }
            "epoch" => proof.binding.epoch = SecurityEpoch::new(1),
            "reference" => proof.reference = BindingDigest::new([99; 32]),
            _ => {}
        }
        let before = store.try_read_committed().unwrap();
        let service = BeginPasskeyEnrollment {
            uow: &uow,
            admins: &admins,
            sessions: &sessions,
            audit: &audit,
            authenticator: &crypto,
            clock: &clock,
            ids: &ids,
            policy: &fixture::policy(),
        };
        let result = service
            .execute(BeginPasskeyEnrollmentInput {
                proof: proof.clone(),
            })
            .await;
        assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
        let after = store.try_read_committed().unwrap();
        assert!(after.admin_sessions.is_empty());
        assert!(after.admin_credentials.is_empty());
        assert_eq!(after.admins, before.admins);
        if scenario == "valid" {
            let result = result.unwrap();
            assert_eq!(
                result.public_options,
                fixture::secret("synthetic-public-options-1")
            );
            assert_eq!(result.binding.stage, AuthStage::Passkey);
            assert_eq!(after.audit_entries.len(), 1);
            assert_eq!(after.admin_pending.len(), 2);
            assert!(
                service
                    .execute(BeginPasskeyEnrollmentInput { proof })
                    .await
                    .is_err()
            );
            let repeated = store.try_read_committed().unwrap();
            assert_eq!(repeated.admin_pending, after.admin_pending);
            assert_eq!(repeated.audit_entries, after.audit_entries);
        } else {
            assert_eq!(after.admin_pending, before.admin_pending);
            assert_eq!(after.audit_entries, before.audit_entries);
        }
    }
}
