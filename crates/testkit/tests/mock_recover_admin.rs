//! T181: one caller-owned transaction, sorted authority locks, complete revocation and safe audit.
use admin::{
    application::recover_admin::{RecoverAdmin, RecoverAdminInput},
    domain::{
        admin_state::{EnrollmentPurpose, Invalidation},
        audit::{Actor, AuditAction, AuditMetadata, ReasonCode, Subject},
    },
    ports::{
        admin_credential_repository::MockAdminCredentialRepository,
        admin_repository::MockAdminRepository, audit_repository::MockAuditRepository,
        otp_challenge_repository::MockOtpChallengeRepository, session_store::MockSessionStore,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ports::{
        clock::MockClock,
        id_generator::MockIdGenerator,
        unit_of_work::{MockUnitOfWork, StoreId, TransactionMode, TxHandle},
    },
};
use std::sync::{Arc, Mutex};
#[path = "support/recovery_fixture.rs"]
mod fixture;

#[tokio::test]
async fn recovery_only_revokes_target_admin() {
    for scenario in [
        "valid",
        "reason",
        "self",
        "actor",
        "stale_actor",
        "missing",
        "epoch",
        "sessions",
        "credentials",
        "pending",
        "otp",
        "authorization",
        "audit",
        "commit",
    ] {
        drive(scenario).await;
    }
}
#[tokio::test]
async fn recovery_races_with_login_safely_state_contract() {
    for scenario in ["valid", "stale_actor", "epoch", "otp", "audit"] {
        drive(scenario).await;
    }
}
#[tokio::test]
async fn audit_recovery_has_complete_safe_fields_once() {
    drive("valid").await;
    drive("audit").await;
}
fn error() -> DomainError {
    DomainError::unavailable(ErrorCode::PrerequisiteUnavailable)
}
fn step(
    events: &Mutex<Vec<&'static str>>,
    name: &'static str,
    fail: bool,
) -> Result<(), DomainError> {
    events.lock().unwrap().push(name);
    if fail { Err(error()) } else { Ok(()) }
}
#[expect(
    clippy::too_many_lines,
    reason = "independent failure injection at every ordered transactional recovery boundary"
)]
async fn drive(scenario: &str) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let store = StoreId::new();
    let reason = scenario != "reason";
    let actor = reason && scenario != "actor";
    let locked = actor && scenario != "self";
    let writes = locked && !matches!(scenario, "stale_actor" | "missing");
    let stages = [
        "epoch",
        "sessions",
        "credentials",
        "pending",
        "otp",
        "authorization",
        "audit",
    ];
    let reached = |name| {
        writes
            && stages.iter().position(|s| *s == name).unwrap()
                <= stages
                    .iter()
                    .position(|s| *s == scenario)
                    .unwrap_or(stages.len())
    };
    let mut uow = MockUnitOfWork::new();
    let log = events.clone();
    uow.expect_begin()
        .times(usize::from(reason))
        .returning(move |mode| {
            assert_eq!(mode, TransactionMode::ReadWrite);
            log.lock().unwrap().push("begin");
            Ok(TxHandle::new(store, mode, ()))
        });
    let log = events.clone();
    let fail = scenario == "commit";
    uow.expect_commit()
        .times(usize::from(matches!(scenario, "valid" | "commit")))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            step(&log, "commit", fail)
        });
    let log = events.clone();
    uow.expect_rollback()
        .times(usize::from(
            reason && !matches!(scenario, "valid" | "commit"),
        ))
        .returning(move |mut tx| {
            tx.take::<()>(store)?;
            step(&log, "rollback", false)
        });
    let mut sessions = MockSessionStore::new();
    let log = events.clone();
    let fail = scenario == "actor";
    sessions
        .expect_check_current()
        .times(usize::from(reason))
        .returning(move |tx, digest, now| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(digest, fixture::digest(7));
            assert_eq!(now, fixture::now());
            step(&log, "actor", fail)?;
            Ok(fixture::session(&fixture::actor(), 20, true))
        });
    let mut admins = MockAdminRepository::new();
    let log = events.clone();
    let stale = scenario == "stale_actor";
    let missing = scenario == "missing";
    admins
        .expect_lock()
        .times(usize::from(locked))
        .returning(move |tx, ids| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(ids, vec![fixture::target().id(), fixture::actor().id()]);
            log.lock().unwrap().push("lock");
            let mut actor = fixture::actor();
            if stale {
                actor
                    .invalidate(
                        actor.epoch(),
                        actor.epoch().advance().unwrap(),
                        Invalidation::Recover,
                    )
                    .unwrap();
            }
            Ok(if missing {
                vec![actor]
            } else {
                vec![fixture::target(), actor]
            })
        });
    let log = events.clone();
    let fail = scenario == "epoch";
    admins
        .expect_invalidate_epoch()
        .times(usize::from(reached("epoch")))
        .returning(move |tx, target, expected, next, kind| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(target, fixture::target().id());
            assert_eq!(expected, fixture::target().epoch());
            assert_eq!(next, expected.advance().unwrap());
            assert_eq!(kind, Invalidation::Recover);
            step(&log, "epoch", fail)
        });
    let log = events.clone();
    let fail = scenario == "sessions";
    sessions
        .expect_revoke_all()
        .times(usize::from(reached("sessions")))
        .returning(move |tx, target, now| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(target, fixture::target().id());
            assert_eq!(now, fixture::now());
            step(&log, "sessions", fail)
        });
    let mut credentials = MockAdminCredentialRepository::new();
    let log = events.clone();
    let fail = scenario == "credentials";
    credentials
        .expect_revoke_all()
        .times(usize::from(reached("credentials")))
        .returning(move |tx, target, now| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(target, fixture::target().id());
            assert_eq!(now, fixture::now());
            step(&log, "credentials", fail)
        });
    let log = events.clone();
    let fail = scenario == "pending";
    sessions
        .expect_revoke_pending()
        .times(usize::from(reached("pending")))
        .returning(move |tx, target| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(target, fixture::target().id());
            step(&log, "pending", fail)
        });
    let mut otp = MockOtpChallengeRepository::new();
    let log = events.clone();
    let fail = scenario == "otp";
    otp.expect_revoke_all()
        .times(usize::from(reached("otp")))
        .returning(move |tx, target| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(target, fixture::target().id());
            step(&log, "otp", fail)
        });
    let log = events.clone();
    let fail = scenario == "authorization";
    admins
        .expect_authorize_enrollment()
        .times(usize::from(reached("authorization")))
        .returning(move |tx, command| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(command.actor, fixture::actor().id());
            assert_eq!(command.target, fixture::target().id());
            assert_eq!(command.purpose, EnrollmentPurpose::Recovery);
            assert_eq!(
                command.expected_epoch,
                fixture::target().epoch().advance().unwrap()
            );
            let mut invalidated = fixture::target();
            invalidated
                .invalidate(
                    invalidated.epoch(),
                    command.expected_epoch,
                    Invalidation::Recover,
                )
                .unwrap();
            assert_eq!(
                command.expected_authorization.as_ref(),
                invalidated.enrollment()
            );
            assert_eq!(command.authorized_at, fixture::now());
            assert_eq!(
                command.expires_at,
                fixture::now() + time::Duration::minutes(5)
            );
            step(&log, "authorization", fail)
        });
    let mut audit = MockAuditRepository::new();
    let log = events.clone();
    let fail = scenario == "audit";
    audit
        .expect_append()
        .times(usize::from(reached("audit")))
        .returning(move |tx, entry| {
            assert_eq!(tx.store_id(), store);
            assert_eq!(entry.actor(), Actor::Admin(fixture::actor().id()));
            assert_eq!(entry.subject(), Subject::Admin(fixture::target().id()));
            assert_eq!(entry.action(), AuditAction::Recovered);
            assert_eq!(entry.reason(), ReasonCode::AdminRecovery);
            assert_eq!(entry.occurred_at(), fixture::now());
            assert_eq!(
                entry.metadata(),
                &AuditMetadata::Eligibility {
                    before: true,
                    after: false
                }
            );
            assert!(!format!("{entry:?}").contains("secret"));
            step(&log, "audit", fail)
        });
    let mut clock = MockClock::new();
    clock.expect_now().returning(|| Ok(fixture::now()));
    let mut ids = MockIdGenerator::new();
    ids.expect_next_audit_id()
        .times(usize::from(reason))
        .returning(|| Ok("019a53a0-0000-7000-8000-000000000099".parse().unwrap()));
    let result = RecoverAdmin {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        credentials: &credentials,
        otp: &otp,
        audit: &audit,
        clock: &clock,
        ids: &ids,
        policy: &fixture::policy(),
    }
    .execute(RecoverAdminInput {
        actor: fixture::digest(7),
        target: if scenario == "self" {
            fixture::actor().id()
        } else {
            fixture::target().id()
        },
        reason: reason.then_some(ReasonCode::AdminRecovery),
    })
    .await;
    assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
    if scenario == "valid" {
        assert_eq!(result.unwrap(), ());
    }
    let mut expected = Vec::new();
    if reason {
        expected.extend(["begin", "actor"]);
        if locked {
            expected.push("lock");
        }
        for name in stages {
            if reached(name) {
                expected.push(name);
            }
        }
        expected.push(if matches!(scenario, "valid" | "commit") {
            "commit"
        } else {
            "rollback"
        });
    }
    assert_eq!(*events.lock().unwrap(), expected, "{scenario}");
}
