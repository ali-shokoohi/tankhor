//! T190: access-form capabilities are crypto-issued, browser-bound and bounded.
use admin::{
    application::prepare_access_form::PrepareAccessForm,
    domain::{auth_state::BindingDigest, secrets::SecretBytes},
    ports::admin_authenticator::{MockAdminAuthenticator, TokenPurpose},
};

#[tokio::test]
async fn access_form_capability_is_browser_bound_not_an_auth_grant_port_interactions() {
    let browser = SecretBytes::new(b"synthetic-browser-capability".to_vec());
    let csrf = SecretBytes::new(b"synthetic-csrf-capability".to_vec());
    let browser_digest = BindingDigest::new([19; 32]);
    let mut authenticator = MockAdminAuthenticator::new();
    let expected_browser = browser.clone();
    authenticator
        .expect_token_digest()
        .times(1)
        .returning(move |purpose, token| {
            assert_eq!(purpose, TokenPurpose::Browser);
            assert_eq!(token, expected_browser);
            Ok(BindingDigest::new([19; 32]))
        });
    let issued = csrf.clone();
    authenticator
        .expect_issue_token()
        .times(1)
        .returning(move |purpose| {
            assert_eq!(purpose, TokenPurpose::Csrf);
            Ok(issued.clone())
        });

    let form = PrepareAccessForm {
        authenticator: &authenticator,
    }
    .execute(browser)
    .await
    .unwrap();

    assert_eq!(form.browser, browser_digest);
    assert_eq!(form.csrf, csrf);
}
