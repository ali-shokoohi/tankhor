//! T188: sign-out affects only the presented current admin session, never revives it.
use admin::{
    application::sign_out::SignOut,
    domain::secrets::SecretBytes,
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        session_store::SessionRecord,
    },
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    clock::FixedClock, session_store::FakeSessionStore, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};
#[path = "support/list_admins_fixture.rs"]
mod fixture;

#[tokio::test]
async fn signout_revokes_current_admin_session_only() {
    for scenario in ["valid", "missing", "incomplete", "expired", "end_user"] {
        drive(scenario).await;
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "independent current/other-session isolation, retry and unchanged-state scenarios"
)]
async fn drive(scenario: &str) {
    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let crypto = FakeAdminAuthenticator::controlled_fixture();
    let clock = FixedClock::new(fixture::now());
    let cookie = crypto.issue_token(TokenPurpose::Session).await.unwrap();
    let digest = crypto
        .token_digest(TokenPurpose::Session, cookie.clone())
        .await
        .unwrap();
    store.seed(|state| {
        state
            .admins
            .insert(fixture::admin(1).id(), fixture::admin(1));
        state
            .admins
            .insert(fixture::admin(2).id(), fixture::admin(2));
        if scenario != "missing" {
            state.admin_sessions.insert(
                *digest.as_bytes(),
                SessionRecord {
                    digest: digest.clone(),
                    state: fixture::session(scenario != "incomplete"),
                },
            );
        }
        let mut input = admin::domain::session_state::SessionInput {
            id: "019a53a0-0000-7000-8000-000000000005".parse().unwrap(),
            admin_id: fixture::admin(2).id(),
            epoch: fixture::admin(2).epoch(),
            primary_verified: true,
            factor: Some(admin::domain::audit::Factor::Totp),
            issued_at: fixture::now(),
            expires_at: fixture::now() + time::Duration::minutes(5),
            idle_timeout: time::Duration::MINUTE,
        };
        state.admin_sessions.insert(
            *fixture::digest().as_bytes(),
            SessionRecord {
                digest: fixture::digest(),
                state: input.clone().try_into().unwrap(),
            },
        );
        input.admin_id = fixture::admin(1).id();
        input.id = "019a53a0-0000-7000-8000-000000000006".parse().unwrap();
        state.admin_sessions.insert(
            [8; 32],
            SessionRecord {
                digest: admin::domain::auth_state::BindingDigest::new([8; 32]),
                state: input.try_into().unwrap(),
            },
        );
    });
    if scenario == "expired" {
        clock.advance(time::Duration::MINUTE);
    }
    let before = store.try_read_committed().unwrap();
    let service = SignOut {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        authenticator: &crypto,
        clock: &clock,
    };
    let result = service
        .execute(if scenario == "end_user" {
            SecretBytes::new(b"synthetic-end-user".to_vec())
        } else {
            cookie.clone()
        })
        .await;
    assert_eq!(result.is_ok(), scenario == "valid", "{scenario}");
    let after = store.try_read_committed().unwrap();
    let mut expected = before.admin_sessions.clone();
    if scenario == "valid" {
        expected
            .get_mut(digest.as_bytes())
            .unwrap()
            .state
            .revoke(fixture::now());
    }
    assert_eq!(after.admin_sessions, expected);
    assert_eq!(after.admins, before.admins);
    assert_eq!(after.audit_entries, before.audit_entries);
    assert_eq!(after.admin_credentials, before.admin_credentials);
    assert_eq!(after.admin_pending, before.admin_pending);
    assert_eq!(after.admin_otps, before.admin_otps);
    if scenario == "valid" {
        let _ = service.execute(cookie).await;
        assert_eq!(
            store.try_read_committed().unwrap().admin_sessions,
            after.admin_sessions
        );
        assert!(
            after.admin_sessions[digest.as_bytes()]
                .state
                .check_access(&fixture::admin(1), fixture::now())
                .is_err()
        );
        after.admin_sessions[fixture::digest().as_bytes()]
            .state
            .check_access(&fixture::admin(2), fixture::now())
            .unwrap();
        after.admin_sessions[&[8; 32]]
            .state
            .check_access(&fixture::admin(1), fixture::now())
            .unwrap();
    }
}
