//! Owned detached PostgreSQL 16 startup and SQL readiness (T014; FR-024/FR-CON-37).

use std::{collections::BTreeMap, os::unix::fs::PermissionsExt, path::Path, time::Duration};

use sqlx::{Connection, PgConnection, postgres::PgConnectOptions};
use testcontainers::{ImageExt, ReuseDirective, core::Mount, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

use super::ownership::{Endpoint, FixtureScope};

/// Fixed diagnostic categories never include connection options or Docker environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresError {
    Credentials,
    Docker,
    Ownership,
    Readiness,
}

impl std::fmt::Display for PostgresError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "PostgreSQL fixture failure: {self:?}")
    }
}

impl std::error::Error for PostgresError {}

/// A ready detached service. Dropping this handle never removes the shared container.
pub struct PostgresFixture {
    id: String,
    endpoint: Endpoint,
    options: PgConnectOptions,
}

impl PostgresFixture {
    /// Start/reuse exactly labelled PostgreSQL and wait for an authenticated SQL round trip.
    ///
    /// # Errors
    /// Refuses unprotected credentials, foreign ownership, non-loopback ports, Docker
    /// failures and services that do not become SQL-ready within the readiness deadline.
    pub async fn start_or_reuse(
        scope: &FixtureScope,
        username: &str,
        password_file: &Path,
    ) -> Result<Self, PostgresError> {
        let metadata =
            std::fs::symlink_metadata(password_file).map_err(|_| PostgresError::Credentials)?;
        if !metadata.is_file()
            || metadata.permissions().mode() & 0o777 != 0o600
            || username.is_empty()
            || !username
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(PostgresError::Credentials);
        }
        let password =
            std::fs::read_to_string(password_file).map_err(|_| PostgresError::Credentials)?;
        if password.trim().is_empty() {
            return Err(PostgresError::Credentials);
        }
        let password_file = password_file
            .canonicalize()
            .map_err(|_| PostgresError::Credentials)?;
        let password_path = password_file.to_str().ok_or(PostgresError::Credentials)?;
        wait_for_unpaused(scope).await?;
        let request = Postgres::default()
            .with_user(username)
            .with_password("")
            .with_tag("16")
            .with_env_var("POSTGRES_PASSWORD_FILE", "/run/secrets/postgres-password")
            .with_mount(Mount::bind_mount(
                password_path,
                "/run/secrets/postgres-password",
            ))
            .with_labels(scope.postgres_labels())
            .with_mapped_port(0, 5432.into())
            .with_host_config_modifier(|config| {
                if let Some(bindings) = &mut config.port_bindings {
                    for ports in bindings.values_mut().flatten() {
                        for port in ports {
                            port.host_ip = Some("127.0.0.1".to_owned());
                        }
                    }
                }
            })
            .with_reuse(ReuseDirective::Always);
        let container = request.start().await.map_err(|_| PostgresError::Docker)?;
        let id = container.id().to_owned();
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .map_err(|_| PostgresError::Docker)?;
        let endpoint = Endpoint::new("127.0.0.1", port).map_err(|_| PostgresError::Ownership)?;

        // Inspect only allowlisted nonsecret metadata. Never emit Docker's full inspect
        // result: it includes the image's environment.
        let mut labels = BTreeMap::new();
        for key in scope.postgres_labels().keys() {
            let format = format!("{{{{index .Config.Labels \"{key}\"}}}}");
            let actual = inspect(&id, &format).await?;
            labels.insert(key.clone(), actual);
        }
        scope
            .validate(&labels, endpoint, endpoint)
            .map_err(|_| PostgresError::Ownership)?;
        let host = inspect(&id, "{{range $p, $bindings := .NetworkSettings.Ports}}{{range $bindings}}{{.HostIp}}{{end}}{{end}}")
            .await?;
        if host != "127.0.0.1" {
            return Err(PostgresError::Ownership);
        }
        let options = PgConnectOptions::new()
            .host("127.0.0.1")
            .port(port)
            .username(username)
            .password(password.trim())
            .database("postgres");
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let attempt = tokio::time::timeout(Duration::from_secs(1), async {
                    let mut connection = PgConnection::connect_with(&options).await?;
                    let result = sqlx::query_scalar::<_, i32>("SELECT 1")
                        .fetch_one(&mut connection)
                        .await;
                    connection.close().await?;
                    result
                })
                .await;
                if matches!(attempt, Ok(Ok(1))) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .map_err(|_| PostgresError::Readiness)?;
        Ok(Self {
            id,
            endpoint,
            options,
        })
    }

    /// Docker resource identity, used for exact-resource inspection and cleanup.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Validated loopback endpoint.
    #[must_use]
    pub fn endpoint(&self) -> Endpoint {
        self.endpoint
    }

    /// Authenticated connection options; callers must never log or export them.
    #[must_use]
    pub fn connect_options(&self) -> PgConnectOptions {
        self.options.clone()
    }
}

// Testcontainers attempts to start a paused reusable container, which Docker refuses.
// Wait for this owned resource to resume; do not return early or create a replacement.
async fn wait_for_unpaused(scope: &FixtureScope) -> Result<(), PostgresError> {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let mut command = tokio::process::Command::new("docker");
            command.args(["ps", "-a", "--format", "{{.ID}}"]);
            for (key, value) in scope.postgres_labels() {
                command.args(["--filter", &format!("label={key}={value}")]);
            }
            let result = command.output().await.map_err(|_| PostgresError::Docker)?;
            if !result.status.success() {
                return Err(PostgresError::Docker);
            }
            let ids = String::from_utf8(result.stdout).map_err(|_| PostgresError::Docker)?;
            let ids: Vec<_> = ids.lines().collect();
            match ids.as_slice() {
                [] => return Ok(()),
                [id] => {
                    if inspect(id, "{{.State.Paused}}").await? != "true" {
                        return Ok(());
                    }
                }
                _ => return Err(PostgresError::Ownership),
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .map_err(|_| PostgresError::Readiness)?
}

async fn inspect(id: &str, format: &str) -> Result<String, PostgresError> {
    let result = tokio::process::Command::new("docker")
        .args(["inspect", "--format", format, id])
        .output()
        .await
        .map_err(|_| PostgresError::Docker)?;
    if !result.status.success() {
        return Err(PostgresError::Docker);
    }
    String::from_utf8(result.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| PostgresError::Docker)
}
