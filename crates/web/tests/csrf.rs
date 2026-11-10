use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::middleware::from_fn_with_state;
use axum::routing::get;
use kernel::locale::Locale;
use tower::ServiceExt;
use web::i18n::ConsoleCatalog;
use web::security::{
    CookieKind, CookieSecurityPolicy, CsrfContext, CsrfProtectionState, enforce_csrf_middleware,
    format_cookie_header,
};

fn assert_cookie_transport_policy() {
    let prod_policy = CookieSecurityPolicy::production();
    let ceremony_cookie =
        format_cookie_header(CookieKind::PrimaryCeremony, "opaque-ceremony", prod_policy).unwrap();
    let session_cookie =
        format_cookie_header(CookieKind::AdminSession, "opaque-session", prod_policy).unwrap();
    assert_ne!(
        CookieKind::PrimaryCeremony.cookie_name(prod_policy),
        CookieKind::AdminSession.cookie_name(prod_policy)
    );
    for header_val in [&ceremony_cookie, &session_cookie] {
        assert!(header_val.starts_with("__Host-"));
        assert!(header_val.contains("; Path=/"));
        assert!(header_val.contains("; HttpOnly"));
        assert!(header_val.contains("; Secure"));
        assert!(header_val.contains("; SameSite=Lax"));
        assert!(!header_val.to_ascii_lowercase().contains("domain="));
    }

    assert!(CookieSecurityPolicy::validate_raw_attributes(true, false, "/", None).is_err());
    assert!(CookieSecurityPolicy::validate_raw_attributes(true, true, "/console", None).is_err());
    assert!(
        CookieSecurityPolicy::validate_raw_attributes(true, true, "/", Some("tankhor.example"))
            .is_err()
    );
}

async fn assert_post_rejections(app: &Router, expected_token: &str) {
    let no_ctx_req = Request::builder()
        .method("POST")
        .uri("/__test/mutate")
        .header(header::ORIGIN, "https://tankhor.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("csrf_token={expected_token}")))
        .unwrap();
    assert_eq!(
        app.clone().oneshot(no_ctx_req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    let mut missing_origin_req = Request::builder()
        .method("POST")
        .uri("/__test/mutate")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("csrf_token={expected_token}")))
        .unwrap();
    missing_origin_req
        .extensions_mut()
        .insert(CsrfContext::new(expected_token));
    assert_eq!(
        app.clone()
            .oneshot(missing_origin_req)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    let mut wrong_origin_req = Request::builder()
        .method("POST")
        .uri("/__test/mutate")
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("csrf_token={expected_token}")))
        .unwrap();
    wrong_origin_req
        .extensions_mut()
        .insert(CsrfContext::new(expected_token));
    assert_eq!(
        app.clone()
            .oneshot(wrong_origin_req)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    let mut wrong_token_req = Request::builder()
        .method("POST")
        .uri("/__test/mutate")
        .header(header::ORIGIN, "https://tankhor.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("csrf_token=wrong-token-value&note=hello"))
        .unwrap();
    wrong_token_req
        .extensions_mut()
        .insert(CsrfContext::new(expected_token));
    let wrong_token_resp = app.clone().oneshot(wrong_token_req).await.unwrap();
    assert_eq!(wrong_token_resp.status(), StatusCode::FORBIDDEN);
    let err_body = String::from_utf8(
        to_bytes(wrong_token_resp.into_body(), 64 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(err_body.contains("نشست فرم منقضی یا نامعتبر است"));
    assert!(!err_body.contains(expected_token));
}

#[tokio::test]
async fn csrf_is_required_for_every_mutating_console_form() {
    let catalog = ConsoleCatalog::load_authoritative().expect("catalog");
    let state = CsrfProtectionState::new("https://tankhor.example", catalog, Locale::Fa)
        .expect("valid origin");

    let app = Router::new()
        .route(
            "/__test/mutate",
            get(|| async { "readonly-get" }).post(|| async { "mutated-ok" }),
        )
        .layer(from_fn_with_state(state, enforce_csrf_middleware));

    let expected_token = "synthetic-browser-bound-token-0123456789abcdef";

    let get_req = Request::builder()
        .method("GET")
        .uri("/__test/mutate")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(get_req).await.unwrap().status(),
        StatusCode::OK
    );

    assert_post_rejections(&app, expected_token).await;

    let mut valid_form_req = Request::builder()
        .method("POST")
        .uri("/__test/mutate")
        .header(header::ORIGIN, "https://tankhor.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!("csrf_token={expected_token}&note=safe")))
        .unwrap();
    valid_form_req
        .extensions_mut()
        .insert(CsrfContext::new(expected_token));
    assert_eq!(
        app.clone().oneshot(valid_form_req).await.unwrap().status(),
        StatusCode::OK
    );

    let mut valid_htmx_req = Request::builder()
        .method("POST")
        .uri("/__test/mutate")
        .header(header::REFERER, "https://tankhor.example/console")
        .header("X-CSRF-Token", expected_token)
        .header("HX-Request", "true")
        .body(Body::empty())
        .unwrap();
    valid_htmx_req
        .extensions_mut()
        .insert(CsrfContext::new(expected_token));
    assert_eq!(
        app.oneshot(valid_htmx_req).await.unwrap().status(),
        StatusCode::OK
    );

    assert_cookie_transport_policy();
}
