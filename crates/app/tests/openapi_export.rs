use std::process::Command;

use app::openapi::export_openapi_json;
use serde_json::Value;
use web::api::openapi::render_openapi_json;

#[test]
fn offline_openapi_export_needs_no_database() {
    // 1. Direct library export requires zero runtime env vars, DB, S3, or SMS configuration
    //    and matches `web`'s canonical OpenAPI builder byte-for-byte.
    let exported = export_openapi_json().expect("offline OpenAPI export must succeed");
    let canonical = render_openapi_json().expect("canonical web OpenAPI render must succeed");
    assert_eq!(exported, canonical);

    let parsed: Value = serde_json::from_str(&exported).expect("exported OpenAPI must be JSON");
    assert!(
        parsed["openapi"]
            .as_str()
            .is_some_and(|v| v.starts_with("3.1.")),
        "expected OpenAPI 3.1.x, got {:?}",
        parsed["openapi"]
    );
    assert!(parsed["paths"]["/api/v1/health"]["get"].is_object());
    assert!(parsed["paths"]["/api/v1/openapi.json"]["get"].is_object());

    // 2. CLI subprocess (`app openapi`) succeeds with cleared/hostile environment variables
    //    (missing `TANKHOR_ENV`, poisoned `DATABASE_URL`, poisoned `S3_ENDPOINT`), proving
    //    zero database/provider/transport-policy connection or runtime config acquisition occurs.
    let bin = env!("CARGO_BIN_EXE_app");
    let output = Command::new(bin)
        .arg("openapi")
        .env_remove("TANKHOR_ENV")
        .env_remove("TANKHOR_ADAPTERS")
        .env_remove("TANKHOR_HTTP_MAX_BODY_BYTES")
        .env_remove("TANKHOR_HTTP_MAX_INFLIGHT")
        .env(
            "DATABASE_URL",
            "postgres://invalid:invalid@127.0.0.1:1/unreachable",
        )
        .env("S3_ENDPOINT", "http://127.0.0.1:1")
        .output()
        .expect("must invoke `app openapi` binary");

    assert!(
        output.status.success(),
        "`app openapi` failed with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout must be valid UTF-8");
    assert_eq!(stdout, canonical);

    // 3. Unknown CLI subcommand fails cleanly with non-zero exit code without panicking.
    let unknown = Command::new(bin)
        .arg("nonexistent-subcommand")
        .env_remove("TANKHOR_ENV")
        .output()
        .expect("must invoke `app` with unknown subcommand");
    assert!(!unknown.status.success());
}
