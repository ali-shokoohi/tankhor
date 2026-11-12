use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;
use web::{console::auth::passkey_bridge_router, middleware::csp::CSP_POLICY_VALUE};

#[tokio::test]
async fn local_passkey_bridge_serializes_binary_fields_without_inline_execution() {
    let response = passkey_bridge_router()
        .oneshot(
            Request::get("/console/static/passkey.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/javascript; charset=utf-8"
    );
    let script = String::from_utf8(
        to_bytes(response.into_body(), 64 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    for required in [
        "navigator.credentials.get",
        "base64url",
        "ArrayBuffer",
        "response",
        "requestSubmit",
        "textContent",
    ] {
        assert!(
            script.contains(required),
            "missing bridge behavior: {required}"
        );
    }
    for forbidden in [
        "eval(",
        "new Function",
        "protected_state",
        "server_state",
        "innerHTML",
    ] {
        assert!(
            !script.contains(forbidden),
            "unsafe bridge content: {forbidden}"
        );
    }
    assert!(CSP_POLICY_VALUE.contains("script-src 'self'"));
    assert!(!CSP_POLICY_VALUE.contains("unsafe-inline"));
}
