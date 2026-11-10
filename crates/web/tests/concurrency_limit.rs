use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::get;
use kernel::locale::Locale;
use tokio::sync::{Notify, oneshot};
use tower::ServiceExt;
use web::api::problem::{PROBLEM_CONTENT_TYPE, ProblemDetails};
use web::i18n::ConsoleCatalog;
use web::middleware::concurrency_limit::{
    ConcurrencyLimitState, enforce_concurrency_limit_middleware,
};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn concurrency_limit_rejects_excess_and_releases_permits() {
    let catalog = Arc::new(ConsoleCatalog::load_authoritative().unwrap());
    let fa_busy = catalog
        .format(Locale::Fa, "error-service-busy", &[])
        .unwrap();
    let en_busy = catalog
        .format(Locale::En, "error-service-busy", &[])
        .unwrap();

    let entered = Arc::new(AtomicUsize::new(0));
    let slot1_entered = Arc::new(Notify::new());
    let slot2_entered = Arc::new(Notify::new());
    let release_gate = Arc::new(Notify::new());

    let limit_state = ConcurrencyLimitState::new(
        NonZeroUsize::new(2).unwrap(),
        Arc::clone(&catalog),
        Locale::Fa,
    );

    let entered_slow = Arc::clone(&entered);
    let slot1_slow = Arc::clone(&slot1_entered);
    let slot2_slow = Arc::clone(&slot2_entered);
    let gate_slow = Arc::clone(&release_gate);

    let app = Router::new()
        .route(
            "/console/slow",
            get(move || {
                let entered = Arc::clone(&entered_slow);
                let s1 = Arc::clone(&slot1_slow);
                let s2 = Arc::clone(&slot2_slow);
                let gate = Arc::clone(&gate_slow);
                async move {
                    let count = entered.fetch_add(1, Ordering::SeqCst) + 1;
                    if count == 1 {
                        s1.notify_waiters();
                    } else if count == 2 {
                        s2.notify_waiters();
                    }
                    gate.notified().await;
                    "ok"
                }
            }),
        )
        .route(
            "/api/v1/fast",
            get(|| async { (StatusCode::OK, "{\"status\":\"ok\"}") }),
        )
        .route(
            "/console/fail",
            get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "boom").into_response() }),
        )
        .layer(middleware::from_fn_with_state(
            limit_state.clone(),
            enforce_concurrency_limit_middleware,
        ));

    // 1. Occupy both configured permits (`max_inflight = 2`).
    let app1 = app.clone();
    let t1 = tokio::spawn(async move {
        app1.oneshot(
            Request::builder()
                .method("GET")
                .uri("/console/slow")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    slot1_entered.notified().await;

    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    let app2 = app.clone();
    let t2 = tokio::spawn(async move {
        tokio::select! {
            res = app2.oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/console/slow")
                    .body(Body::empty())
                    .unwrap(),
            ) => Some(res.unwrap()),
            _ = cancel_rx => None,
        }
    });
    slot2_entered.notified().await;
    assert_eq!(entered.load(Ordering::SeqCst), 2);
    assert_eq!(limit_state.available_permits(), 0);

    // 2. Excess console HTML request is immediately rejected with 503 localized HTML
    //    without entering the handler (`entered` remains 2).
    let excess_fa = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/console/slow")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(excess_fa.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        entered.load(Ordering::SeqCst),
        2,
        "excess request must never enter the handler"
    );
    let fa_bytes = to_bytes(excess_fa.into_body(), 16 * 1024).await.unwrap();
    let fa_html = String::from_utf8(fa_bytes.to_vec()).unwrap();
    assert!(fa_html.contains(&fa_busy));
    assert!(fa_html.contains("lang=\"fa\""));

    // English `?lang=en` console overload returns localized English 503 HTML.
    let excess_en = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/console/slow?lang=en")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(excess_en.status(), StatusCode::SERVICE_UNAVAILABLE);
    let en_bytes = to_bytes(excess_en.into_body(), 16 * 1024).await.unwrap();
    let en_html = String::from_utf8(en_bytes.to_vec()).unwrap();
    assert!(en_html.contains(&en_busy));
    assert!(en_html.contains("lang=\"en\""));

    // 3. Excess `/api/v1` request is immediately rejected with 503 `application/problem+json`.
    let excess_api = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/fast")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(excess_api.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        excess_api
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        PROBLEM_CONTENT_TYPE
    );
    let api_bytes = to_bytes(excess_api.into_body(), 16 * 1024).await.unwrap();
    let problem: ProblemDetails = serde_json::from_slice(&api_bytes).unwrap();
    assert_eq!(problem.status, 503);
    assert_eq!(
        problem.problem_type,
        "urn:tankhor:problem:service-overloaded"
    );
    assert_eq!(problem.instance, "/api/v1/fast");

    // 4. Cancel/drop in-flight task `t2`: its permit must be released immediately on drop.
    let _ = cancel_tx.send(());
    assert!(t2.await.unwrap().is_none());
    assert_eq!(
        limit_state.available_permits(),
        1,
        "cancelled/dropped request future must release its in-flight permit"
    );

    // 5. Error response (`/console/fail` -> 500) acquires and releases its permit cleanly.
    let fail_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/console/fail")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fail_resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        limit_state.available_permits(),
        1,
        "error response must release its in-flight permit"
    );

    // 6. Release remaining slow request `t1` and verify all permits (`2`) are restored.
    release_gate.notify_waiters();
    let resp1 = t1.await.unwrap();
    assert_eq!(resp1.status(), StatusCode::OK);
    assert_eq!(limit_state.available_permits(), 2);

    // Subsequent `/api/v1/fast` succeeds cleanly.
    let ok_api = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/fast")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok_api.status(), StatusCode::OK);
}
