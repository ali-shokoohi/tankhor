use std::collections::HashMap;
use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::middleware;
use scraper::{Html, Selector};
use testkit::fakes::health_probe::FakeHealthProbe;
use testkit::fakes::store::FakeStore;
use tower::ServiceExt;
use web::api::docs::build_api_and_docs_router;
use web::api::openapi::SchemaAccessPolicy;
use web::i18n::ConsoleCatalog;
use web::middleware::csp::{CSP_POLICY_VALUE, apply_csp_middleware};
use web::shell::shell_router;

fn parse_csp_directives(header_value: &str) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    for raw_directive in header_value.split(';') {
        let trimmed = raw_directive.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut tokens = trimmed.split_whitespace();
        let name = tokens.next().unwrap().to_ascii_lowercase();
        let values = tokens.map(str::to_owned).collect::<Vec<_>>();
        map.insert(name, values);
    }
    map
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "One CSP scenario checks headers and executable markup across all foundation surfaces"
)]
async fn csp_allows_only_local_console_scripts() {
    let store = FakeStore::new();
    store.seed(|state| {
        state.health_ready = true;
    });
    let (api_docs, _) = build_api_and_docs_router(
        Arc::new(FakeHealthProbe::new(store)),
        SchemaAccessPolicy::NonProductionOpen,
    );
    let app = shell_router(ConsoleCatalog::load_authoritative().unwrap())
        .merge(api_docs)
        .layer(middleware::from_fn(apply_csp_middleware));

    // 1. Inspect Content-Security-Policy header on console HTML, JSON API, and Swagger UI.
    for uri in ["/console", "/api/v1/health", "/api/docs/"] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "URI `{uri}` failed");

        let csp = resp
            .headers()
            .get(header::CONTENT_SECURITY_POLICY)
            .expect("every response must carry Content-Security-Policy")
            .to_str()
            .unwrap();
        assert_eq!(csp, CSP_POLICY_VALUE);

        let directives = parse_csp_directives(csp);
        assert_eq!(directives.get("default-src").unwrap(), &vec!["'self'"]);
        assert_eq!(directives.get("object-src").unwrap(), &vec!["'none'"]);
        assert_eq!(directives.get("base-uri").unwrap(), &vec!["'none'"]);
        assert_eq!(directives.get("frame-ancestors").unwrap(), &vec!["'none'"]);
        assert_eq!(directives.get("form-action").unwrap(), &vec!["'self'"]);
        assert_eq!(directives.get("connect-src").unwrap(), &vec!["'self'"]);
        assert_eq!(directives.get("font-src").unwrap(), &vec!["'self'"]);
        assert_eq!(directives.get("img-src").unwrap(), &vec!["'self'", "data:"]);
        assert_eq!(directives.get("script-src").unwrap(), &vec!["'self'"]);
        assert_eq!(directives.get("style-src").unwrap(), &vec!["'self'"]);

        let lowered = csp.to_ascii_lowercase();
        for forbidden in [
            "unsafe-inline",
            "unsafe-eval",
            "https:",
            "http:",
            "*",
            "validator.swagger.io",
            "unpkg.com",
        ] {
            assert!(
                !lowered.contains(forbidden),
                "CSP must forbid `{forbidden}`: {csp}"
            );
        }

        // Verify companion hardening headers.
        assert_eq!(
            resp.headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .unwrap()
                .to_str()
                .unwrap(),
            "nosniff"
        );
        assert_eq!(
            resp.headers()
                .get(header::REFERRER_POLICY)
                .unwrap()
                .to_str()
                .unwrap(),
            "same-origin"
        );
    }

    // 2. Verify console HTML and Swagger HTML contain zero inline <script>, zero inline <style>,
    //    zero inline event handler attributes (`on*`), and zero `style=` attributes, while
    //    local same-origin scripts (htmx, Swagger bundle/initializer, and future `/static/...`
    //    passkey/dialog scripts) remain permitted under `script-src 'self'`.
    for html_uri in ["/console", "/api/docs/"] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(html_uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(resp.into_body(), 256 * 1024).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        let doc = Html::parse_document(&html);

        let script_sel = Selector::parse("script").unwrap();
        for script in doc.select(&script_sel) {
            assert!(
                script.inner_html().trim().is_empty(),
                "inline script body forbidden under strict CSP on `{html_uri}`"
            );
            let src = script
                .value()
                .attr("src")
                .expect("script tag must have same-origin src");
            assert!(
                src.starts_with('/') || src.starts_with("./"),
                "non-local script src `{src}` forbidden on `{html_uri}`"
            );
        }

        let style_sel = Selector::parse("style").unwrap();
        assert_eq!(
            doc.select(&style_sel).count(),
            0,
            "inline <style> element forbidden under `style-src 'self'` on `{html_uri}`"
        );

        let all_sel = Selector::parse("*").unwrap();
        for el in doc.select(&all_sel) {
            for (attr_name, _) in el.value().attrs() {
                let lower = attr_name.to_ascii_lowercase();
                assert!(
                    !lower.starts_with("on"),
                    "inline event handler `{attr_name}` forbidden on `{html_uri}`"
                );
                assert_ne!(
                    lower, "style",
                    "inline style attribute forbidden under `style-src 'self'` on `{html_uri}`"
                );
            }
        }
    }
}
