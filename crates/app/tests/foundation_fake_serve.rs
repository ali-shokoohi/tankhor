//! T102: Sandboxed debug fake-mode foundation `serve` smoke test (FR-001, FR-025, FR-CON-01, FR-UX-01).

mod support {
    pub mod network_sandbox;
}

use std::process::Stdio;

use serde_json::Value;
use support::network_sandbox::OutboundNetworkSandbox;

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "The sandboxed process lifecycle and foundation smoke assertions form one end-to-end scenario"
)]
fn fake_foundation_shell_and_health_start_without_network() {
    // 1. Verify host sandbox capability first (setup blocker if host lacks unshare/loopback).
    let sandbox = OutboundNetworkSandbox::prepare_verified()
        .expect("host network sandbox setup must succeed before running business assertions");
    let bin = env!("CARGO_BIN_EXE_app");

    // 2. Production + fake mode MUST be rejected before resource acquisition.
    let prod_reject = sandbox
        .command(bin, &["serve"])
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("TANKHOR_ENV", "production")
        .env("TANKHOR_ADAPTERS", "fake")
        .env("TANKHOR_HTTP_BIND", "127.0.0.1:0")
        .env("TANKHOR_HTTP_MAX_BODY_BYTES", "65536")
        .env("TANKHOR_HTTP_MAX_INFLIGHT", "32")
        .output()
        .expect("spawn production rejection check");
    assert!(
        !prod_reject.status.success(),
        "production environment must reject TANKHOR_ADAPTERS=fake before startup"
    );

    // 3. Start `app serve` inside the verified outbound-network-denial namespace
    //    with NO DATABASE_URL, NO S3_*, and NO SMS_* variables configured.
    //    Because `unshare -n` creates a private network namespace per child, the HTTP client
    //    runs inside the same sandboxed script once `app serve` announces its bound address,
    //    proving zero external routes exist for the entire lifetime of `app serve`.
    let smoke_probe = r#"
set -eu
ip link set lo up
BIN="$1"
python3 - "$BIN" <<'PY'
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

bin_path = sys.argv[1]
env = {
    "PATH": os.environ.get("PATH", ""),
    "TANKHOR_ENV": "development",
    "TANKHOR_ADAPTERS": "fake",
    "TANKHOR_HTTP_BIND": "127.0.0.1:0",
    "TANKHOR_HTTP_MAX_BODY_BYTES": "65536",
    "TANKHOR_HTTP_MAX_INFLIGHT": "32",
}

proc = subprocess.Popen(
    [bin_path, "serve"],
    env=env,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
)

try:
    ready_line = ""
    deadline = time.monotonic() + 5.0
    while time.monotonic() < deadline:
        line = proc.stdout.readline()
        if line:
            if line.startswith("TANKHOR_LISTENING "):
                ready_line = line.strip()
                break
        elif proc.poll() is not None:
            break

    if not ready_line:
        rc = proc.poll()
        stderr_out = proc.stderr.read() if proc.stderr else ""
        print(json.dumps({"started": False, "returncode": rc, "stderr": stderr_out}))
        sys.exit(11)

    addr = ready_line.split(" ", 1)[1].strip()
    base = f"http://{addr}"

    def fetch(path: str):
        req = urllib.request.Request(base + path, method="GET")
        try:
            with urllib.request.urlopen(req, timeout=3.0) as resp:
                return {
                    "status": resp.status,
                    "headers": dict(resp.headers.items()),
                    "body": resp.read().decode("utf-8"),
                }
        except urllib.error.HTTPError as err:
            return {
                "status": err.code,
                "headers": dict(err.headers.items()),
                "body": err.read().decode("utf-8"),
            }

    results = {
        "started": True,
        "addr": addr,
        "console_fa": fetch("/console"),
        "console_en": fetch("/console?lang=en&theme=dark"),
        "health": fetch("/api/v1/health"),
        "openapi": fetch("/api/v1/openapi.json"),
        "swagger": fetch("/api/docs/"),
        "absent_settings": fetch("/console/settings"),
        "absent_flags": fetch("/console/flags"),
        "absent_audit": fetch("/console/audit"),
        "absent_api_settings": fetch("/api/v1/settings"),
    }
    print(json.dumps(results))
finally:
    proc.terminate()
    try:
        proc.wait(timeout=3.0)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=3.0)
PY
"#;

    let output = std::process::Command::new("unshare")
        .args(["-Urn", "sh", "-ceu", smoke_probe, "--", bin])
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .output()
        .expect("execute sandboxed foundation serve probe");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "fake-mode foundation `app serve` failed inside outbound-network-denial sandbox (status={}): stdout={stdout} stderr={stderr}",
        output.status
    );

    let report: Value = serde_json::from_str(stdout.trim()).expect("probe JSON output");
    assert_eq!(report["started"], true);

    // Verify Persian default shell (`lang="fa"`, `dir="rtl"`, CSP header).
    assert_eq!(report["console_fa"]["status"], 200);
    let fa_body = report["console_fa"]["body"].as_str().expect("fa HTML");
    assert!(fa_body.contains(r#"lang="fa""#) && fa_body.contains(r#"dir="rtl""#));
    assert!(fa_body.contains("تن\u{200c}خور"));
    let fa_csp = report["console_fa"]["headers"]
        .as_object()
        .and_then(|h| {
            h.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content-security-policy"))
                .and_then(|(_, v)| v.as_str())
        })
        .expect("CSP header must be present on /console");
    assert!(fa_csp.contains("default-src 'self'"));
    assert!(!fa_csp.contains("unsafe-inline"));

    // Verify English dark shell (`lang="en"`, `dir="ltr"`, `data-theme="dark"`).
    assert_eq!(report["console_en"]["status"], 200);
    let en_body = report["console_en"]["body"].as_str().expect("en HTML");
    assert!(
        en_body.contains(r#"lang="en""#)
            && en_body.contains(r#"dir="ltr""#)
            && en_body.contains(r#"data-theme="dark""#)
    );
    assert!(en_body.contains("Tankhor"));

    // Unseeded fake health MUST fail closed with 503 application/problem+json (never fabricated 200).
    assert_eq!(
        report["health"]["status"], 503,
        "unseeded fake health must return 503 ProblemDetails, never fabricated ready"
    );
    let health_ct = report["health"]["headers"]
        .as_object()
        .and_then(|h| {
            h.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                .and_then(|(_, v)| v.as_str())
        })
        .expect("health content-type");
    assert!(health_ct.starts_with("application/problem+json"));
    let health_problem: Value =
        serde_json::from_str(report["health"]["body"].as_str().expect("health body"))
            .expect("problem+json payload");
    assert_eq!(health_problem["status"], 503);
    assert_eq!(
        health_problem["type"],
        "urn:tankhor:problem:service-unavailable"
    );

    // OpenAPI and vendored Swagger UI work in development fake mode without external network.
    assert_eq!(report["openapi"]["status"], 200);
    assert_eq!(report["swagger"]["status"], 200);

    // Undelivered management routes MUST remain unregistered (404), never placeholder stubs.
    for absent_key in [
        "absent_settings",
        "absent_flags",
        "absent_audit",
        "absent_api_settings",
    ] {
        assert_eq!(
            report[absent_key]["status"], 404,
            "undelivered route `{absent_key}` must return 404, not a placeholder handler"
        );
    }
}
