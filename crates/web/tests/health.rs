use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::Value;
use testkit::fakes::health_probe::FakeHealthProbe;
use testkit::fakes::store::FakeStore;
use tower::ServiceExt;
use web::api::health_api_router;

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "Ready and unavailable responses are checked against the same health schema scenario"
)]
async fn health_and_problem_responses_match_schema() {
    // 1. Seeded fake store -> 200 OK {"status": "ok"} under /api/v1/health.
    let seeded_store = FakeStore::new();
    seeded_store.seed(|state| {
        state.health_ready = true;
    });
    let (ready_router, openapi_doc) =
        health_api_router(Arc::new(FakeHealthProbe::new(seeded_store)));

    let ok_resp = ready_router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok_resp.status(), StatusCode::OK);
    let ok_ct = ok_resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ok_ct.starts_with("application/json"));
    let ok_json: Value =
        serde_json::from_slice(&to_bytes(ok_resp.into_body(), 16 * 1024).await.unwrap()).unwrap();
    assert_eq!(ok_json, serde_json::json!({ "status": "ok" }));

    // Non-v1 path `/api/health` is NOT registered.
    let non_v1_resp = ready_router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(non_v1_resp.status(), StatusCode::NOT_FOUND);

    // 2. Unseeded store -> 503 Service Unavailable with RFC 9457 application/problem+json.
    let (unseeded_router, _) = health_api_router(Arc::new(FakeHealthProbe::new(FakeStore::new())));
    let unseeded_resp = unseeded_router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unseeded_resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        unseeded_resp
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/problem+json"
    );
    let problem_bytes = to_bytes(unseeded_resp.into_body(), 16 * 1024)
        .await
        .unwrap();
    let problem_text = String::from_utf8(problem_bytes.to_vec()).unwrap();
    let problem: Value = serde_json::from_str(&problem_text).unwrap();
    assert_eq!(problem["status"], 503);
    assert!(
        problem["type"]
            .as_str()
            .unwrap()
            .starts_with("urn:tankhor:problem:")
    );
    assert!(!problem["title"].as_str().unwrap().is_empty());
    assert!(!problem["detail"].as_str().unwrap().is_empty());
    assert!(!problem["instance"].as_str().unwrap().is_empty());
    for forbidden in ["postgres", "sqlx", "password", "secret", "127.0.0.1"] {
        assert!(
            !problem_text.to_ascii_lowercase().contains(forbidden),
            "problem response leaked `{forbidden}`: {problem_text}"
        );
    }

    // 3. Unavailable probe -> 503 application/problem+json.
    let (unavailable_router, _) = health_api_router(Arc::new(FakeHealthProbe::unavailable()));
    let unavail_resp = unavailable_router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unavail_resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        unavail_resp
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/problem+json"
    );

    // 4. OpenAPI document generated from OpenApiRouter documents /api/v1/health 200 & 503.
    let doc_value = serde_json::to_value(&openapi_doc).unwrap();
    let paths = doc_value["paths"].as_object().unwrap();
    assert!(paths.contains_key("/api/v1/health"));
    let get_op = &paths["/api/v1/health"]["get"];
    assert!(get_op["responses"]["200"].is_object());
    assert!(get_op["responses"]["503"].is_object());
    assert_eq!(
        get_op["responses"]["503"]["content"]["application/problem+json"]["schema"]["$ref"],
        "#/components/schemas/ProblemDetails"
    );
}
