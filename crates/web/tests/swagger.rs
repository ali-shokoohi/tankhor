use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use scraper::{Html, Selector};
use serde_json::Value;
use testkit::fakes::health_probe::FakeHealthProbe;
use testkit::fakes::store::FakeStore;
use tower::ServiceExt;
use web::api::docs::build_api_and_docs_router;
use web::api::openapi::SchemaAccessPolicy;

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn swagger_makes_no_external_asset_or_validator_requests() {
    let store = FakeStore::new();
    store.seed(|state| {
        state.health_ready = true;
    });
    let (router, doc) = build_api_and_docs_router(
        Arc::new(FakeHealthProbe::new(store)),
        SchemaAccessPolicy::NonProductionOpen,
    );

    // 1. `/api/docs` responds locally (200 HTML directly or local redirect to `/api/docs/`).
    let initial = router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/docs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (docs_path, docs_resp) = if initial.status().is_redirection() {
        let loc = initial
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        assert!(
            loc.starts_with('/'),
            "Swagger redirect must stay same-origin: {loc}"
        );
        let target = if loc.starts_with("/api/docs") {
            loc
        } else {
            format!("/api/{}", loc.trim_start_matches('/'))
        };
        let followed = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&target)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        (target, followed)
    } else {
        ("/api/docs".to_owned(), initial)
    };

    assert_eq!(
        docs_resp.status(),
        StatusCode::OK,
        "path `{docs_path}` failed"
    );
    let html_bytes = to_bytes(docs_resp.into_body(), 256 * 1024).await.unwrap();
    let html_text = String::from_utf8(html_bytes.to_vec()).unwrap();

    // No external CDN or validator.swagger.io URLs in HTML.
    for forbidden in [
        "http://",
        "https://",
        "//unpkg.com",
        "//cdn.",
        "validator.swagger.io",
    ] {
        assert!(
            !html_text.to_ascii_lowercase().contains(forbidden),
            "Swagger HTML referenced external URL `{forbidden}`: {html_text}"
        );
    }

    let document = Html::parse_document(&html_text);
    let script_sel = Selector::parse("script").unwrap();
    let link_sel = Selector::parse("link[rel='stylesheet']").unwrap();

    let mut script_srcs = Vec::new();
    for script in document.select(&script_sel) {
        assert!(
            script.inner_html().trim().is_empty(),
            "Swagger HTML must use local script files with zero inline <script> body for strict CSP"
        );
        let src = script
            .value()
            .attr("src")
            .expect("every <script> in Swagger HTML must carry a local src");
        assert!(
            !src.contains("://") && !src.starts_with("//"),
            "external script src forbidden: {src}"
        );
        script_srcs.push(src.to_owned());
    }
    assert!(
        !script_srcs.is_empty(),
        "Swagger HTML must reference local bundle and initializer scripts"
    );

    let mut css_hrefs = Vec::new();
    for link in document.select(&link_sel) {
        let href = link
            .value()
            .attr("href")
            .expect("stylesheet link must have href");
        assert!(
            !href.contains("://") && !href.starts_with("//"),
            "external stylesheet href forbidden: {href}"
        );
        css_hrefs.push(href.to_owned());
    }
    assert!(
        !css_hrefs.is_empty(),
        "Swagger HTML must reference local vendored stylesheet"
    );

    let resolve_asset_uri = |raw: &str| -> String {
        if raw.starts_with('/') {
            raw.to_owned()
        } else {
            format!("/api/docs/{}", raw.trim_start_matches("./"))
        }
    };

    // Every referenced local JS and CSS asset resolves locally with 200 OK, and initializer
    // configures `/api/v1/openapi.json` with external validation disabled (`validatorUrl: null`).
    let mut all_js = String::new();
    for src in &script_srcs {
        let uri = resolve_asset_uri(src);
        let asset_resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            asset_resp.status(),
            StatusCode::OK,
            "local Swagger script `{uri}` failed to load"
        );
        let js_bytes = to_bytes(asset_resp.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap();
        all_js.push_str(&String::from_utf8_lossy(&js_bytes));
        all_js.push('\n');
    }

    assert!(
        all_js.contains("/api/v1/openapi.json"),
        "Swagger initializer must point directly at `/api/v1/openapi.json`"
    );
    assert!(
        all_js.contains("\"validatorUrl\": \"none\"")
            || all_js.contains("validatorUrl: \"none\"")
            || all_js.contains("validatorUrl: null"),
        "Swagger initializer must explicitly disable external validator (`validatorUrl: \"none\"`): {all_js}"
    );
    assert!(
        !all_js.contains("https://petstore.swagger.io"),
        "Swagger initializer must not retain upstream petstore demo URL"
    );

    for href in &css_hrefs {
        let uri = resolve_asset_uri(href);
        let css_resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            css_resp.status(),
            StatusCode::OK,
            "local Swagger stylesheet `{uri}` failed to load"
        );
    }

    // Swagger mounting must NOT register any undocumented extra JSON schema route;
    // OpenAPI paths remain strictly `/api/v1/health` and `/api/v1/openapi.json`.
    let doc_json: Value = serde_json::to_value(&doc).unwrap();
    let documented_paths: Vec<String> = doc_json["paths"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(documented_paths.len(), 2);
    assert!(documented_paths.contains(&"/api/v1/health".to_owned()));
    assert!(documented_paths.contains(&"/api/v1/openapi.json".to_owned()));

    for extra_schema in [
        "/api/openapi.json",
        "/api/docs/openapi.json",
        "/api-docs/openapi.json",
    ] {
        let extra_resp = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(extra_schema)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_ne!(
            extra_resp.status(),
            StatusCode::OK,
            "Swagger UI must not expose an undocumented extra schema endpoint at `{extra_schema}`"
        );
    }
}
