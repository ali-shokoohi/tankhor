//! T158: fake-backed access is granted only by an atomic completed second factor.
use admin::{
    application::passkey::{BeginPasskey, BeginPasskeyInput, FinishPasskey, FinishPasskeyInput},
    domain::{
        admin_state::Invalidation,
        audit::{Actor, AuditAction, AuditMetadata, Factor, ReasonCode, Subject},
        auth_state::{AuthStage, BindingDigest},
    },
    ports::{
        admin_authenticator::{
            AdminAuthenticator, MockAdminAuthenticator, PasskeyChallenge, PasskeyFinish,
            TokenPurpose,
        },
        session_store::SessionStore,
    },
};
use kernel::ports::unit_of_work::{TransactionMode, UnitOfWork};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator,
    admin_credential_repository::FakeAdminCredentialRepository,
    admin_repository::FakeAdminRepository, audit_repository::FakeAuditRepository,
    clock::FixedClock, id_generator::SequenceIdGenerator, session_store::FakeSessionStore,
    store::FakeStore, unit_of_work::FakeUnitOfWork,
};
#[path = "support/passkey_fixture.rs"]
mod fixture;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Success,
    Invalid,
    Epoch,
    Credential,
    Audit,
    WrongBrowser,
    Expired,
    CasRace,
    StateRace,
    ExpiryRace,
    EpochRace,
}

#[expect(
    clippy::too_many_lines,
    reason = "independent end-to-end ceremony scenarios with committed snapshot assertions"
)]
async fn drive(outcome: Outcome) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let credentials = FakeAdminCredentialRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let audit = FakeAuditRepository::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = std::sync::Arc::new(FixedClock::new(fixture::now()));
    let ids = SequenceIdGenerator::new(100);
    let policy = fixture::policy();
    store.seed(|state| {
        state.admins.insert(fixture::admin().id(), fixture::admin());
        state
            .admin_credentials
            .insert(fixture::credential_id(), fixture::credential());
        state
            .admin_pending
            .insert([5; 32], fixture::pending(AuthStage::PrimaryVerified));
    });
    let begin = BeginPasskey {
        uow: &uow,
        admins: &admins,
        credentials: &credentials,
        sessions: &sessions,
        authenticator: &crypto,
        clock: clock.as_ref(),
        ids: &ids,
        policy: &policy,
    };
    let primary = fixture::check(AuthStage::PrimaryVerified);
    let started = begin
        .execute(BeginPasskeyInput {
            proof: primary.clone(),
            credential: fixture::credential_id(),
        })
        .await
        .unwrap();
    assert!(
        begin
            .execute(BeginPasskeyInput {
                proof: primary,
                credential: fixture::credential_id()
            })
            .await
            .is_err()
    );
    assert!(
        store
            .try_read_committed()
            .unwrap()
            .admin_sessions
            .is_empty()
    );
    let reference = crypto
        .token_digest(TokenPurpose::Ceremony, started.token.clone())
        .await
        .unwrap();
    let pending = store.try_read_committed().unwrap().admin_pending[reference.as_bytes()].clone();
    let mut request = fixture::start();
    request.context.challenge = pending.state().id();
    request.context.binding = started.binding.clone();
    let mut tx = uow.begin(TransactionMode::ReadOnly).await.unwrap();
    let check = admin::ports::session_store::PendingCheck {
        reference,
        binding: started.binding.clone(),
        now: fixture::now(),
    };
    let protected_state = sessions
        .verification_state(&mut tx, check.clone())
        .await
        .unwrap();
    uow.commit(tx).await.unwrap();
    let response = crypto
        .passkey_fixture(
            &request,
            &PasskeyChallenge {
                public_options: started.public_options,
                protected_state: protected_state.clone(),
            },
        )
        .unwrap();
    match outcome {
        Outcome::Epoch => store.seed(|state| {
            let actor = state.admins.get_mut(&fixture::admin().id()).unwrap();
            actor
                .invalidate(
                    actor.epoch(),
                    actor.epoch().advance().unwrap(),
                    Invalidation::Recover,
                )
                .unwrap();
        }),
        Outcome::Credential => store.seed(|state| {
            let old = state.admin_credentials[&fixture::credential_id()].clone();
            let mut next = old.state().clone();
            next.advance_revision(next.epoch(), 1, 2).unwrap();
            state.admin_credentials.insert(
                fixture::credential_id(),
                admin::ports::admin_credential_repository::CredentialRecord::new(
                    next,
                    old.material().clone(),
                )
                .unwrap(),
            );
        }),
        Outcome::Audit => audit.fail_next_append(),
        Outcome::Expired => clock.advance(time::Duration::MINUTE),
        _ => {}
    }
    let mut race_crypto = MockAdminAuthenticator::new();
    let race = matches!(
        outcome,
        Outcome::CasRace | Outcome::StateRace | Outcome::ExpiryRace | Outcome::EpochRace
    );
    if race {
        let verified = crypto
            .finish_passkey(PasskeyFinish {
                request,
                protected_state,
                response: response.clone(),
                now: fixture::now(),
            })
            .await
            .unwrap();
        let race_store = store.clone();
        let race_check = check.clone();
        let race_clock = clock.clone();
        race_crypto
            .expect_finish_passkey()
            .times(1)
            .return_once(move |_| {
                match outcome {
                    Outcome::CasRace => race_store.seed(|state| {
                        let old = state.admin_credentials[&fixture::credential_id()].clone();
                        let mut next = old.state().clone();
                        next.advance_revision(next.epoch(), 1, 2).unwrap();
                        state.admin_credentials.insert(
                            fixture::credential_id(),
                            admin::ports::admin_credential_repository::CredentialRecord::new(
                                next,
                                old.material().clone(),
                            )
                            .unwrap(),
                        );
                    }),
                    Outcome::StateRace => race_store.seed(|state| {
                        state
                            .admin_pending
                            .get_mut(race_check.reference.as_bytes())
                            .unwrap()
                            .consume(&race_check, &fixture::admin())
                            .unwrap();
                    }),
                    Outcome::EpochRace => race_store.seed(|state| {
                        let actor = state.admins.get_mut(&fixture::admin().id()).unwrap();
                        actor
                            .invalidate(
                                actor.epoch(),
                                actor.epoch().advance().unwrap(),
                                Invalidation::Recover,
                            )
                            .unwrap();
                    }),
                    Outcome::ExpiryRace => race_clock.advance(time::Duration::MINUTE),
                    _ => unreachable!(),
                }
                Ok(verified)
            });
        let token = crypto.issue_token(TokenPurpose::Session).await.unwrap();
        let digest = crypto
            .token_digest(TokenPurpose::Session, token.clone())
            .await
            .unwrap();
        race_crypto
            .expect_issue_token()
            .returning(move |_| Ok(token.clone()));
        race_crypto
            .expect_token_digest()
            .returning(move |_, _| Ok(digest.clone()));
    }
    let before = store.try_read_committed().unwrap();
    let finish = FinishPasskey {
        uow: &uow,
        admins: &admins,
        credentials: &credentials,
        sessions: &sessions,
        audit: &audit,
        authenticator: if race { &race_crypto } else { &crypto },
        clock: clock.as_ref(),
        ids: &ids,
        policy: &policy,
    };
    let mut input = FinishPasskeyInput {
        proof: check.clone(),
        credential: fixture::credential_id(),
        response,
    };
    if outcome == Outcome::Invalid {
        input.response = fixture::secret("unregistered-proof");
    }
    if outcome == Outcome::WrongBrowser {
        input.proof.binding.browser = BindingDigest::new([99; 32]);
    }
    let result = finish.execute(input.clone()).await;
    assert_eq!(result.is_ok(), outcome == Outcome::Success);
    let after = store.try_read_committed().unwrap();
    if outcome == Outcome::Success {
        assert_eq!(after.admin_sessions.len(), 1);
        assert_eq!(after.audit_entries.len(), 1);
        let record = &after.admin_credentials[&fixture::credential_id()];
        assert_eq!(record.state().revision(), 2);
        assert!(
            matches!(record.material(),admin::ports::admin_credential_repository::CredentialMaterial::Passkey{credential,..} if *credential==crypto.verified_passkey().credential)
        );
        let entry = after.audit_entries.values().next().unwrap();
        assert_eq!(entry.actor(), Actor::Admin(fixture::admin().id()));
        assert_eq!(entry.subject(), Subject::Admin(fixture::admin().id()));
        assert_eq!(entry.action(), AuditAction::SignedIn);
        assert_eq!(entry.occurred_at(), fixture::now());
        assert_eq!(entry.reason(), ReasonCode::Authentication);
        assert_eq!(entry.metadata(), &AuditMetadata::Factor(Factor::Passkey));
        assert!(!format!("{entry:?}").contains("synthetic"));
        let token = result.unwrap().token;
        let digest = crypto
            .token_digest(TokenPurpose::Session, token)
            .await
            .unwrap();
        assert!(
            after.admin_sessions[digest.as_bytes()]
                .state
                .check_access(&fixture::admin(), fixture::now())
                .is_ok()
        );
        assert!(finish.execute(input).await.is_err());
        let replay = store.try_read_committed().unwrap();
        assert_eq!(replay.audit_entries, after.audit_entries);
        assert_eq!(replay.admin_sessions, after.admin_sessions);
        assert_eq!(replay.admin_credentials, after.admin_credentials);
    } else {
        assert_eq!(after.admin_sessions, before.admin_sessions);
        assert_eq!(after.audit_entries, before.audit_entries);
        if outcome != Outcome::CasRace {
            assert_eq!(after.admin_credentials, before.admin_credentials);
        }
        if outcome != Outcome::StateRace {
            assert_eq!(after.admin_pending, before.admin_pending);
        }
        if outcome == Outcome::CasRace {
            assert_eq!(
                after.admin_credentials[&fixture::credential_id()]
                    .state()
                    .revision(),
                2
            );
            assert_eq!(
                after.admin_credentials[&fixture::credential_id()].material(),
                fixture::credential().material()
            );
        }
    }
}
#[tokio::test]
async fn console_requires_completed_second_factor() {
    for outcome in [
        Outcome::Success,
        Outcome::Invalid,
        Outcome::Epoch,
        Outcome::Credential,
        Outcome::WrongBrowser,
        Outcome::Expired,
    ] {
        drive(outcome).await;
    }
}
#[tokio::test]
async fn passkey_consumption_is_single_use_under_race_state_contract() {
    for outcome in [
        Outcome::Success,
        Outcome::CasRace,
        Outcome::StateRace,
        Outcome::ExpiryRace,
        Outcome::EpochRace,
    ] {
        drive(outcome).await;
    }
}
#[tokio::test]
async fn audit_failure_aborts_access_state_contract() {
    drive(Outcome::Audit).await;
}
#[tokio::test]
async fn audit_passkey_signin_has_complete_safe_fields_once() {
    drive(Outcome::Success).await;
}
