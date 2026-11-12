//! T191: a form/CSRF capability is neither primary proof nor console access.
use admin::{
    application::{console_access::VerifyConsoleAccess, prepare_access_form::PrepareAccessForm},
    ports::admin_authenticator::{AdminAuthenticator, TokenPurpose},
};
use testkit::fakes::{
    admin_authenticator::FakeAdminAuthenticator, admin_repository::FakeAdminRepository,
    clock::FixedClock, session_store::FakeSessionStore, store::FakeStore,
    unit_of_work::FakeUnitOfWork,
};

#[tokio::test]
async fn access_form_capability_is_browser_bound_not_an_auth_grant() {
    let authenticator = FakeAdminAuthenticator::controlled_fixture();
    let browser = authenticator
        .issue_token(TokenPurpose::Browser)
        .await
        .unwrap();
    let other_browser = authenticator
        .issue_token(TokenPurpose::Browser)
        .await
        .unwrap();
    let service = PrepareAccessForm {
        authenticator: &authenticator,
    };
    let form = service.execute(browser.clone()).await.unwrap();
    let other_form = service.execute(other_browser).await.unwrap();

    assert_ne!(form.browser, other_form.browser);
    assert_ne!(form.csrf, other_form.csrf);
    assert_eq!(
        form.browser,
        authenticator
            .token_digest(TokenPurpose::Browser, browser.clone())
            .await
            .unwrap()
    );
    authenticator
        .token_digest(TokenPurpose::Csrf, form.csrf.clone())
        .await
        .unwrap();
    assert!(!format!("{form:?}").contains("synthetic-token"));
    assert_ne!(form.csrf.expose(), browser.expose());

    let store = FakeStore::new();
    let uow = FakeUnitOfWork::new(store.clone());
    let sessions = FakeSessionStore::new(store.clone());
    let admins = FakeAdminRepository::new(store.clone());
    let clock = FixedClock::new(time::OffsetDateTime::UNIX_EPOCH);
    let access = VerifyConsoleAccess {
        uow: &uow,
        admins: &admins,
        sessions: &sessions,
        authenticator: &authenticator,
        clock: &clock,
    }
    .execute(form.csrf)
    .await;
    assert!(access.is_err());
    let state = store.try_read_committed().unwrap();
    assert!(state.admin_pending.is_empty());
    assert!(state.admin_sessions.is_empty());
    assert!(state.admin_credentials.is_empty());
    assert!(state.audit_entries.is_empty());
}
