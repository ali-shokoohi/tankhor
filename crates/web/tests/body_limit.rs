use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::middleware;
use axum::routing::post;
use kernel::locale::Locale;
use tower::ServiceExt;
use web::api::problem::{PROBLEM_CONTENT_TYPE, ProblemDetails};
use web::i18n::ConsoleCatalog;
use web::middleware::body_limit::{BodyLimitState, enforce_body_limit_middleware};
use web::security::{CsrfContext, CsrfProtectionState, enforce_csrf_middleware};

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "Boundary and oversized requests share handler counters to prove pre-application rejection"
)]
async fn oversized_body_never_reaches_application() {
    let catalog = ConsoleCatalog::load_authoritative().unwrap();
    let fa_expected_error = catalog
        .format(Locale::Fa, "error-body-too-large", &[])
        .unwrap();
    let en_expected_error = catalog
        .format(Locale::En, "error-body-too-large", &[])
        .unwrap();

    let console_calls = Arc::new(AtomicUsize::new(0));
    let api_calls = Arc::new(AtomicUsize::new(0));

    let csrf_state = CsrfProtectionState::new(
        "https://tankhor.example",
        ConsoleCatalog::load_authoritative().unwrap(),
        Locale::Fa,
    )
    .unwrap();
    let body_limit = BodyLimitState::new(
        NonZeroUsize::new(64).unwrap(),
        Arc::new(catalog),
        Locale::Fa,
    );

    let console_calls_handler = Arc::clone(&console_calls);
    let console_routes = Router::new()
        .route(
            "/console/action",
            post(move |body: axum::body::Bytes| {
                let counter = Arc::clone(&console_calls_handler);
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    format!("accepted:{}", body.len())
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            csrf_state,
            enforce_csrf_middleware,
        ));

    let api_calls_handler = Arc::clone(&api_calls);
    let api_routes = Router::new().route(
        "/api/v1/echo",
        post(move |body: axum::body::Bytes| {
            let counter = Arc::clone(&api_calls_handler);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                format!("{{\"bytes\":{}}}", body.len())
            }
        }),
    );

    let app = console_routes
        .merge(api_routes)
        .layer(middleware::from_fn_with_state(
            body_limit,
            enforce_body_limit_middleware,
        ));

    // 1. Oversized console form (> 64 bytes) is rejected BEFORE application handler runs
    //    with localized Persian HTML 413.
    let oversized_form = format!("csrf_token=tok-1&note={}", "x".repeat(80));
    let mut req_oversized_html = Request::builder()
        .method("POST")
        .uri("/console/action")
        .header(header::ORIGIN, "https://tankhor.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(oversized_form))
        .unwrap();
    req_oversized_html
        .extensions_mut()
        .insert(CsrfContext::new("tok-1"));

    let resp_html = app.clone().oneshot(req_oversized_html).await.unwrap();
    assert_eq!(resp_html.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        console_calls.load(Ordering::SeqCst),
        0,
        "oversized console form must never reach the application handler"
    );
    let html_ct = resp_html
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        html_ct.starts_with("text/html"),
        "console 413 must render HTML, got `{html_ct}`"
    );
    let html_bytes = to_bytes(resp_html.into_body(), 16 * 1024).await.unwrap();
    let html_text = String::from_utf8(html_bytes.to_vec()).unwrap();
    assert!(html_text.contains(&fa_expected_error));
    assert!(html_text.contains("lang=\"fa\""));
    assert!(html_text.contains("dir=\"rtl\""));

    // 2. Oversized console request preferring English (`?lang=en`) receives localized English HTML 413.
    let mut req_oversized_en = Request::builder()
        .method("POST")
        .uri("/console/action?lang=en")
        .header(header::ORIGIN, "https://tankhor.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("x".repeat(65)))
        .unwrap();
    req_oversized_en
        .extensions_mut()
        .insert(CsrfContext::new("tok-1"));
    let resp_en = app.clone().oneshot(req_oversized_en).await.unwrap();
    assert_eq!(resp_en.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(console_calls.load(Ordering::SeqCst), 0);
    let en_bytes = to_bytes(resp_en.into_body(), 16 * 1024).await.unwrap();
    let en_text = String::from_utf8(en_bytes.to_vec()).unwrap();
    assert!(en_text.contains(&en_expected_error));
    assert!(en_text.contains("lang=\"en\""));
    assert!(en_text.contains("dir=\"ltr\""));

    // 3. Oversized `/api/v1` request (> 64 bytes) is rejected BEFORE handler runs with
    //    RFC 9457 `application/problem+json` 413.
    let oversized_json = format!("{{\"payload\":\"{}\"}}", "y".repeat(80));
    let resp_api = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/echo")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(oversized_json))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_api.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        api_calls.load(Ordering::SeqCst),
        0,
        "oversized API payload must never reach the application handler"
    );
    assert_eq!(
        resp_api
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        PROBLEM_CONTENT_TYPE
    );
    let api_bytes = to_bytes(resp_api.into_body(), 16 * 1024).await.unwrap();
    let problem: ProblemDetails = serde_json::from_slice(&api_bytes).unwrap();
    assert_eq!(problem.status, 413);
    assert_eq!(
        problem.problem_type,
        "urn:tankhor:problem:payload-too-large"
    );
    assert_eq!(problem.instance, "/api/v1/echo");

    // 4. Within-limit console form WITHOUT valid CSRF still fails CSRF (403) before handler runs,
    //    proving body-limit middleware preserves downstream CSRF checks.
    let mut req_bad_csrf = Request::builder()
        .method("POST")
        .uri("/console/action")
        .header(header::ORIGIN, "https://tankhor.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from("csrf_token=wrong&note=ok"))
        .unwrap();
    req_bad_csrf
        .extensions_mut()
        .insert(CsrfContext::new("tok-1"));
    let resp_bad_csrf = app.clone().oneshot(req_bad_csrf).await.unwrap();
    assert_eq!(resp_bad_csrf.status(), StatusCode::FORBIDDEN);
    assert_eq!(console_calls.load(Ordering::SeqCst), 0);

    // 5. Exact-boundary (64 bytes) valid CSRF form passes both body limit and CSRF and reaches handler.
    let prefix = "csrf_token=tok-1&note=";
    let exact_body = format!("{prefix}{}", "z".repeat(64 - prefix.len()));
    assert_eq!(exact_body.len(), 64);
    let mut req_valid = Request::builder()
        .method("POST")
        .uri("/console/action")
        .header(header::ORIGIN, "https://tankhor.example")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(exact_body))
        .unwrap();
    req_valid.extensions_mut().insert(CsrfContext::new("tok-1"));
    let resp_valid = app.oneshot(req_valid).await.unwrap();
    assert_eq!(resp_valid.status(), StatusCode::OK);
    assert_eq!(console_calls.load(Ordering::SeqCst), 1);
}
