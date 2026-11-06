//! Cluster-global fixture role provisioning (T016; FR-024/FR-CON-37).

use std::{os::unix::fs::PermissionsExt, path::Path};

use sqlx::{ConnectOptions, Connection, Executor, PgConnection, postgres::PgConnectOptions};

/// Sanitized credential/provisioning error; SQL and password values are never exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleError;

impl std::fmt::Display for RoleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("fixture role provisioning failed")
    }
}

impl std::error::Error for RoleError {}

/// Protected connection material, deliberately without Debug or Display.
pub struct FixtureRoles {
    migration: String,
    runtime: String,
    preparation: String,
}

impl FixtureRoles {
    /// Migration-owner connection, never the runtime connection.
    #[must_use]
    pub fn migration(&self, base: &PgConnectOptions) -> PgConnectOptions {
        base.clone()
            .username("tankhor_fixture_migration")
            .password(&self.migration)
    }

    /// Restricted application connection; it cannot own or create databases/roles.
    #[must_use]
    pub fn runtime(&self, base: &PgConnectOptions) -> PgConnectOptions {
        base.clone()
            .username("tankhor_fixture_runtime")
            .password(&self.runtime)
    }

    /// Separate isolated SQL preparation connection.
    #[must_use]
    pub fn preparation(&self, base: &PgConnectOptions) -> PgConnectOptions {
        base.clone()
            .username("tankhor_fixture_preparation")
            .password(&self.preparation)
    }
}

/// Provision roles once under a cluster-wide transaction lock, safely repeatable by peers.
///
/// # Errors
/// Refuses unprotected/invalid credential files or any role provisioning failure.
pub async fn provision_roles(
    administrator: &PgConnectOptions,
    credentials: &Path,
) -> Result<FixtureRoles, RoleError> {
    let roles = FixtureRoles {
        migration: read_password(&credentials.join("migration"))?,
        runtime: read_password(&credentials.join("runtime"))?,
        preparation: read_password(&credentials.join("preparation"))?,
    };
    let options = administrator.clone().disable_statement_logging();
    let mut connection = PgConnection::connect_with(&options)
        .await
        .map_err(|_| RoleError)?;
    let result = provision(&mut connection, &roles).await;
    let closed = connection.close().await.map_err(|_| RoleError);
    result?;
    closed?;
    Ok(roles)
}

fn read_password(path: &Path) -> Result<String, RoleError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| RoleError)?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(RoleError);
    }
    let password = std::fs::read_to_string(path).map_err(|_| RoleError)?;
    // Locally generated 256-bit hex passwords need no SQL escaping. Reject all other
    // values instead of permitting statement injection through fixture setup material.
    if password.len() != 64 || !password.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RoleError);
    }
    Ok(password)
}

async fn provision(connection: &mut PgConnection, roles: &FixtureRoles) -> Result<(), RoleError> {
    let mut transaction = connection.begin().await.map_err(|_| RoleError)?;
    // Neither successful statements nor failed secret-bearing SQL may enter server logs.
    transaction
        .execute("SET LOCAL log_statement = 'none'")
        .await
        .map_err(|_| RoleError)?;
    transaction
        .execute("SET LOCAL log_min_error_statement = 'panic'")
        .await
        .map_err(|_| RoleError)?;
    sqlx::query("SELECT pg_advisory_xact_lock(738291104)")
        .execute(&mut *transaction)
        .await
        .map_err(|_| RoleError)?;
    for (name, password) in [
        ("tankhor_fixture_migration", &roles.migration),
        ("tankhor_fixture_runtime", &roles.runtime),
        ("tankhor_fixture_preparation", &roles.preparation),
    ] {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = $1)")
                .bind(name)
                .fetch_one(&mut *transaction)
                .await
                .map_err(|_| RoleError)?;
        if !exists {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE ROLE {name} LOGIN")))
                .execute(&mut *transaction)
                .await
                .map_err(|_| RoleError)?;
        }
        // Names are code constants, passwords are constrained hex, and SQLx statement
        // logging is disabled on this dedicated provisioning connection.
        let sql = format!(
            "ALTER ROLE {name} LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE \
            NOREPLICATION NOBYPASSRLS PASSWORD '{password}'"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&mut *transaction)
            .await
            .map_err(|_| RoleError)?;
    }
    transaction.commit().await.map_err(|_| RoleError)
}
