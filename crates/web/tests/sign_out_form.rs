mod support;

use admin::domain::secrets::SecretBytes;
use admin::ports::admin_authenticator::{AdminAuthenticator, TokenPurpose};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use support::{CSRF, Fixture, ORIGIN, seed_console_session};
use tower::ServiceExt;
use web::{
    console::auth::sign_out_router,
    security::{CookieKind, CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
async fn sign_out_form_has_required_authorization_and_bound_state() {
    let fixture = Fixture::new();
    let admin = fixture.services.ids.next_admin_id().await.unwrap();
    let session = fixture.services.ids.next_session_id().await.unwrap();
    let token = seed_console_session(&fixture, admin, session).await;
    let cookies = CookieSecurityPolicy::production();
    let cookie = format!("{}={token}", CookieKind::AdminSession.cookie_name(cookies));
    let app = sign_out_router(fixture.services.clone(), fixture.csrf(), cookies);
    let response = app
        .clone()
        .oneshot(
            Request::post("/console/sign-out")
                .header(header::COOKIE, &cookie)
                .header(header::ORIGIN, ORIGIN)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!("csrf_token={CSRF}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let digest = fixture
        .authenticator
        .token_digest(TokenPurpose::Session, SecretBytes::new(token.into_bytes()))
        .await
        .unwrap();
    let state = fixture.store.try_read_committed().unwrap();
    let session_state = &state.admin_sessions.get(digest.as_bytes()).unwrap().state;
    assert!(
        session_state
            .check_access(
                state.admins.get(&admin).unwrap(),
                time::OffsetDateTime::UNIX_EPOCH
            )
            .is_err()
    );
    drop(state);
    let replay = app
        .oneshot(
            Request::post("/console/sign-out")
                .header(header::COOKIE, cookie)
                .header(header::ORIGIN, ORIGIN)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!("csrf_token={CSRF}")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::UNAUTHORIZED);
}
