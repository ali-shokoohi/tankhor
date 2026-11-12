use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;
use web::console::auth::sign_in_router;
use web::i18n::ConsoleCatalog;

#[tokio::test]
async fn sign_in_page_is_localized_no_store_and_uses_safe_return_target() {
    let authenticator =
        Arc::new(testkit::fakes::admin_authenticator::FakeAdminAuthenticator::controlled_fixture());
    let app = sign_in_router(
        authenticator,
        ConsoleCatalog::load_authoritative().unwrap(),
        web::security::CookieSecurityPolicy::loopback_insecure_dev(),
    );

    for (lang, direction, expected) in [("fa", "rtl", "کد"), ("en", "ltr", "code")] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/console/sign-in?lang={lang}&return=%2Fconsole%2Fadmins"
                    ))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let set_cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("SameSite=Lax"));
        let body = String::from_utf8(
            to_bytes(response.into_body(), 65536)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            body.contains(&format!("lang=\"{lang}\""))
                && body.contains(&format!("dir=\"{direction}\""))
        );
        assert!(body.contains("name=\"csrf_token\"") && body.contains("name=\"phone\""));
        assert!(body.contains("name=\"return\" value=\"/console/admins\""));
        assert!(body.to_lowercase().contains(expected));
        assert!(!body.contains("<script>") && !body.contains("https://"));
    }

    for unsafe_target in [
        "https://evil.example/steal",
        "//evil.example/steal",
        "/%5Cevil",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/console/sign-in?return={unsafe_target}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = String::from_utf8(
            to_bytes(response.into_body(), 65536)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(!body.contains(unsafe_target));
        assert!(body.contains("name=\"return\" value=\"/console/admins\""));
    }
}
