//! T101: Child-process outbound-network-denial harness (FR-001, FR-CON-01).
//!
//! Creates an unprivileged Linux user + network namespace (`unshare -Urn`) with `lo`
//! brought online and zero non-loopback routes or interfaces. Verifies inside the
//! isolated namespace itself that:
//! 1. Loopback listener sockets (`127.0.0.1:0`) bind and accept connections cleanly.
//! 2. Outbound external IPv4/IPv6 socket connections fail immediately at the kernel
//!    routing layer (`ENETUNREACH`).
//!
//! Any missing host capability (`unshare`, `ip`, or user/network namespace restrictions)
//! returns a typed `SandboxSetupError` that aborts setup explicitly rather than silently
//! running unsandboxed or fabricating a red business test result.

use std::fmt;
use std::process::{Command, Stdio};

/// Setup error indicating the host cannot construct or verify the network sandbox.
#[derive(Debug)]
pub struct SandboxSetupError {
    detail: String,
}

impl fmt::Display for SandboxSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "network sandbox setup blocked (missing host isolation capability): {}",
            self.detail
        )
    }
}

impl std::error::Error for SandboxSetupError {}

/// Verified child-process launcher executing inside an isolated loopback-only network namespace.
#[derive(Debug, Clone)]
pub struct OutboundNetworkSandbox {
    _verified: (),
}

impl OutboundNetworkSandbox {
    /// Verifies that the host supports unprivileged user + network namespaces with loopback
    /// listener access and kernel-enforced refusal of external sockets before returning a
    /// ready [`OutboundNetworkSandbox`].
    ///
    /// # Errors
    ///
    /// Returns [`SandboxSetupError`] if `unshare -Urn`, `ip link set lo up`, loopback bind,
    /// or outbound external socket refusal cannot be verified.
    pub fn prepare_verified() -> Result<Self, SandboxSetupError> {
        // Self-test probe run inside the exact `unshare -Urn` wrapper:
        // 1. Bring `lo` up.
        // 2. Bind a TCP listener on 127.0.0.1:0 and connect to it over loopback.
        // 3. Attempt an outbound TCP socket to 198.51.100.1:443 (RFC 5737 TEST-NET-2) and
        //    require immediate OS error (NetworkUnreachable / HostUnreachable / AddrNotAvailable).
        let self_test_script = r#"
set -eu
ip link set lo up
python3 - <<'PY'
import errno
import socket
import sys

listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
listener.bind(("127.0.0.1", 0))
listener.listen(1)
host, port = listener.getsockname()
client = socket.create_connection((host, port), timeout=1.0)
accepted, _ = listener.accept()
client.sendall(b"ok")
assert accepted.recv(2) == b"ok"
accepted.close()
client.close()
listener.close()

external = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
external.settimeout(0.5)
try:
    external.connect(("198.51.100.1", 443))
except OSError as exc:
    if exc.errno in (errno.ENETUNREACH, errno.EHOSTUNREACH, errno.EADDRNOTAVAIL, errno.EACCES, errno.EPERM):
        sys.exit(0)
    print(f"unexpected external socket errno={exc.errno}: {exc}", file=sys.stderr)
    sys.exit(21)
else:
    print("sandbox failed: outbound external connection succeeded", file=sys.stderr)
    sys.exit(22)
finally:
    external.close()
PY
"#;

        let output = Command::new("unshare")
            .args(["-Urn", "sh", "-ceu", self_test_script])
            .stdin(Stdio::null())
            .output()
            .map_err(|err| SandboxSetupError {
                detail: format!("failed to execute `unshare`: {err}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SandboxSetupError {
                detail: format!(
                    "sandbox verification exited with {}: {}",
                    output.status,
                    stderr.trim()
                ),
            });
        }

        Ok(Self { _verified: () })
    }

    /// Builds a [`Command`] that brings `lo` online inside a fresh network namespace and
    /// execs `program` with `args` so the child process has loopback access only and zero
    /// external network routes, plus inherits direct signal delivery via `exec`.
    #[must_use]
    #[allow(clippy::unused_self)]
    pub fn command(&self, program: &str, args: &[&str]) -> Command {
        let mut cmd = Command::new("unshare");
        cmd.args([
            "-Urn",
            "sh",
            "-ceu",
            "ip link set lo up && exec \"$@\"",
            "--",
            program,
        ]);
        cmd.args(args);
        cmd
    }
}
