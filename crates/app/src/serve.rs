//! HTTP server startup for `app serve`.
//!
//! Validates [`AppConfig`] (rejecting `production` + `fake` strictly before any resource or
//! socket acquisition) and [`TransportPolicy`], composes delivered foundational services, binds
//! the local TCP listener, announces readiness, and serves cleanly until shutdown signal.

use std::io::{self, Write};
use std::net::SocketAddr;

use tokio::net::TcpListener;

use crate::composition::{CompositionError, compose_access, compose_foundation};
use crate::config::{AppConfig, ConfigError};
use crate::foundation::{FoundationError, build_foundation_router};
use crate::transport_policy::{TransportPolicy, TransportPolicyError};

/// Environment variable specifying the TCP bind address for `app serve` (defaults to `127.0.0.1:8080`).
pub const ENV_HTTP_BIND: &str = "TANKHOR_HTTP_BIND";

/// Optional environment variable specifying the expected canonical console origin (defaults to
/// `http://<local_addr>` when unset in local development).
pub const ENV_HTTP_ORIGIN: &str = "TANKHOR_HTTP_ORIGIN";

/// Default loopback bind address when [`ENV_HTTP_BIND`] is unset.
pub const DEFAULT_HTTP_BIND: &str = "127.0.0.1:8080";

/// Errors returned by `app serve`.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    /// Application environment or adapter-mode validation failed.
    #[error("{0}")]
    Config(#[from] ConfigError),
    /// Bounded HTTP transport policy validation failed.
    #[error("{0}")]
    TransportPolicy(#[from] TransportPolicyError),
    /// Invalid `TANKHOR_HTTP_BIND` socket address.
    #[error("invalid {ENV_HTTP_BIND} socket address `{value}`")]
    InvalidBindAddress {
        /// Rejected socket address string.
        value: String,
    },
    /// Runtime adapter composition failed.
    #[error("{0}")]
    Composition(#[from] CompositionError),
    /// Foundational router assembly failed.
    #[error("{0}")]
    Foundation(#[from] FoundationError),
    /// Listener bind or server I/O error.
    #[error("HTTP server I/O failure: {0}")]
    Io(#[from] io::Error),
}

/// Validates environment configuration before acquiring any sockets or resources, then binds
/// and runs the foundational HTTP server until `SIGINT` / `SIGTERM`.
///
/// # Errors
///
/// Returns [`ServeError`] if configuration, transport policy, adapter composition, or socket
/// binding fails.
pub async fn run_from_env() -> Result<(), ServeError> {
    // 1. Validate environment & adapter mode FIRST (rejects production + fake before any socket/DB).
    let config = AppConfig::from_env()?;
    let transport_policy = TransportPolicy::from_env()?;
    let bind_raw = std::env::var(ENV_HTTP_BIND).unwrap_or_else(|_| DEFAULT_HTTP_BIND.to_owned());
    let bind_addr: SocketAddr = bind_raw
        .trim()
        .parse()
        .map_err(|_| ServeError::InvalidBindAddress { value: bind_raw })?;
    let configured_origin = std::env::var(ENV_HTTP_ORIGIN).ok();

    // 2. Compose application-wide adapters (no external connections in fake mode).
    let composition = compose_foundation(&config)?;
    let access = compose_access(&config)?;

    // 3. Bind listener and derive expected origin if not explicitly provided.
    let listener = TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;
    let expected_origin = configured_origin
        .filter(|val| !val.trim().is_empty())
        .unwrap_or_else(|| format!("http://{local_addr}"));

    let router = build_foundation_router(
        config.environment(),
        transport_policy,
        &expected_origin,
        composition.health_probe(),
        access.services(),
    )?;

    // Announce bound address and flush stdout immediately so process supervisors/tests observe readiness.
    {
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "TANKHOR_LISTENING {local_addr}")?;
        stdout.flush()?;
    }

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
