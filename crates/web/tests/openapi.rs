use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::Value;
use testkit::fakes::health_probe::FakeHealthProbe;
use testkit::fakes::store::FakeStore;
use tower::ServiceExt;
use web::api::openapi::{SchemaAccessPolicy, build_api_router, build_openapi_document};

#[tokio::test]
async fn schema_endpoint_describes_itself_finitely() {
    let store = FakeStore::new();
    store.seed(|state| {
        state.health_ready = true;
    });
    let (router, frozen_doc) = build_api_router(
        Arc::new(FakeHealthProbe::new(store)),
        SchemaAccessPolicy::NonProductionOpen,
    );

    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(ct.starts_with("application/json"));

    let served_bytes = to_bytes(resp.into_body(), 128 * 1024).await.unwrap();
    let served_json: Value = serde_json::from_slice(&served_bytes).unwrap();
    let frozen_json = serde_json::to_value(&frozen_doc).unwrap();
    let offline_json = serde_json::to_value(build_openapi_document()).unwrap();

    // Served schema, split-router document, and offline builder document are identical.
    assert_eq!(served_json, frozen_json);
    assert_eq!(served_json, offline_json);

    // Top-level OpenAPI 3.1 shape.
    assert!(served_json["openapi"].as_str().unwrap().starts_with("3.1."));
    assert_eq!(served_json["info"]["title"], "Tankhor API");

    // `/api/v1/openapi.json` documents itself via the finite OpenApiDocument schema bridge
    // without embedding a recursive literal copy of the document.
    let openapi_op = &served_json["paths"]["/api/v1/openapi.json"]["get"];
    assert!(openapi_op.is_object());
    assert_eq!(
        openapi_op["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
        "#/components/schemas/OpenApiDocument"
    );
    assert_eq!(
        openapi_op["responses"]["401"]["content"]["application/problem+json"]["schema"]["$ref"],
        "#/components/schemas/ProblemDetails"
    );

    let bridge_schema = &served_json["components"]["schemas"]["OpenApiDocument"];
    assert_eq!(bridge_schema["type"], "object");
    let required: Vec<&str> = bridge_schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for expected_field in ["openapi", "info", "paths"] {
        assert!(
            required.contains(&expected_field),
            "OpenApiDocument schema missing required field `{expected_field}`"
        );
    }
    let bridge_text = serde_json::to_string(bridge_schema).unwrap();
    assert!(
        !bridge_text.contains("/api/v1/openapi.json"),
        "OpenApiDocument schema must not embed a recursive literal copy of paths"
    );

    // Production policy fails closed with 401 problem+json until US1 supplies real admin sessions.
    let (prod_router, _) = build_api_router(
        Arc::new(FakeHealthProbe::unavailable()),
        SchemaAccessPolicy::ProductionAdminRequired,
    );
    let prod_resp = prod_router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prod_resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        prod_resp
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/problem+json"
    );
}

#[tokio::test]
async fn all_json_routes_are_documented_under_v1() {
    let store = FakeStore::new();
    store.seed(|state| {
        state.health_ready = true;
    });
    let (router, doc) = build_api_router(
        Arc::new(FakeHealthProbe::new(store)),
        SchemaAccessPolicy::NonProductionOpen,
    );

    let doc_json = serde_json::to_value(&doc).unwrap();
    let paths: Vec<String> = doc_json["paths"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();

    assert_eq!(
        paths.len(),
        2,
        "Phase 2 JSON API must expose only /api/v1/health and /api/v1/openapi.json: {paths:?}"
    );
    for path in &paths {
        assert!(
            path.starts_with("/api/v1/"),
            "every JSON route must live under /api/v1/: {path}"
        );
    }
    assert!(paths.contains(&"/api/v1/health".to_owned()));
    assert!(paths.contains(&"/api/v1/openapi.json".to_owned()));

    // Undocumented non-v1 aliases must return 404 Not Found.
    for unversioned in [
        "/api/openapi.json",
        "/openapi.json",
        "/api/health",
        "/health",
    ] {
        let resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(unversioned)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "undocumented route `{unversioned}` must not be registered"
        );
    }
}
