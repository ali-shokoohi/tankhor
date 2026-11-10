//! Caller-supplied `SQLx` per-test database ownership, role separation, and cleanup helpers (T062).

use std::path::Path;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sqlx::{ConnectOptions, Connection, Executor, PgConnection};

use super::roles::{FixtureRoles, provision_roles};

/// Sanitized error returned by database fixture verification helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureDatabaseError {
    MissingCredentials,
    ConnectionFailed,
    RoleProvisioningFailed,
    MissingMigrationMetadata,
    PrivilegeSeparationViolated,
    CleanupVerificationFailed,
}

impl std::fmt::Display for FixtureDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCredentials => f.write_str("fixture credential directory is missing"),
            Self::ConnectionFailed => f.write_str("fixture database connection failed"),
            Self::RoleProvisioningFailed => f.write_str("fixture role provisioning failed"),
            Self::MissingMigrationMetadata => {
                f.write_str("canonical SQLx migration metadata table is absent")
            }
            Self::PrivilegeSeparationViolated => {
                f.write_str("runtime role exceeded restricted privilege boundary")
            }
            Self::CleanupVerificationFailed => {
                f.write_str("database pool cleanup verification failed")
            }
        }
    }
}

impl std::error::Error for FixtureDatabaseError {}

/// Caller-supplied `SQLx` per-test database handle with provisioned restricted roles.
pub struct FixtureDatabase {
    database_name: String,
    admin_options: PgConnectOptions,
    admin_pool: PgPool,
    roles: FixtureRoles,
}

impl FixtureDatabase {
    /// Connects to a caller-supplied `SQLx` test database and ensures cluster fixture roles exist.
    ///
    /// # Errors
    /// Returns a sanitized [`FixtureDatabaseError`] if credentials, roles, or connectivity fail.
    pub async fn connect(
        admin_pool_options: PgPoolOptions,
        admin_options: PgConnectOptions,
    ) -> Result<Self, FixtureDatabaseError> {
        let passfile =
            std::env::var_os("PGPASSFILE").ok_or(FixtureDatabaseError::MissingCredentials)?;
        let credential_dir = Path::new(&passfile)
            .parent()
            .ok_or(FixtureDatabaseError::MissingCredentials)?;
        let roles = provision_roles(&admin_options, credential_dir)
            .await
            .map_err(|_| FixtureDatabaseError::RoleProvisioningFailed)?;

        let admin_pool = admin_pool_options
            .max_connections(2)
            .connect_with(admin_options.clone().disable_statement_logging())
            .await
            .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;

        let database_name: String = sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&admin_pool)
            .await
            .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;

        Ok(Self {
            database_name,
            admin_options,
            admin_pool,
            roles,
        })
    }

    #[must_use]
    pub fn database_name(&self) -> &str {
        &self.database_name
    }

    #[must_use]
    pub const fn admin_pool(&self) -> &PgPool {
        &self.admin_pool
    }

    #[must_use]
    pub fn server_admin_options(&self) -> PgConnectOptions {
        self.admin_options
            .clone()
            .database("postgres")
            .disable_statement_logging()
    }

    #[must_use]
    pub fn migration_options(&self) -> PgConnectOptions {
        self.roles
            .migration(&self.admin_options)
            .disable_statement_logging()
    }

    #[must_use]
    pub fn runtime_options(&self) -> PgConnectOptions {
        self.roles
            .runtime(&self.admin_options)
            .disable_statement_logging()
    }

    /// Connects a restricted application runtime pool (`tankhor_fixture_runtime`) to this test DB.
    ///
    /// # Errors
    /// Returns [`FixtureDatabaseError::ConnectionFailed`] if connecting fails.
    pub async fn connect_runtime_pool(
        &self,
        max_connections: u32,
    ) -> Result<PgPool, FixtureDatabaseError> {
        PgPoolOptions::new()
            .max_connections(max_connections)
            .connect_with(self.runtime_options())
            .await
            .map_err(|_| FixtureDatabaseError::ConnectionFailed)
    }

    /// Verifies `_sqlx_migrations` exists in this fresh database and `tankhor_fixture_runtime`
    /// is neither database owner, superuser, `CREATEDB`, `CREATEROLE`, nor able to escalate.
    ///
    /// # Errors
    /// Returns a typed [`FixtureDatabaseError`] if migration metadata or role isolation fails.
    pub async fn verify_migrated_and_role_separation(&self) -> Result<(), FixtureDatabaseError> {
        let has_migrations_table: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'public' AND table_name = '_sqlx_migrations'
            )",
        )
        .fetch_one(&self.admin_pool)
        .await
        .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;
        if !has_migrations_table {
            return Err(FixtureDatabaseError::MissingMigrationMetadata);
        }

        let db_owner: String = sqlx::query_scalar(
            "SELECT pg_catalog.pg_get_userbyid(d.datdba)
             FROM pg_catalog.pg_database d
             WHERE d.datname = current_database()",
        )
        .fetch_one(&self.admin_pool)
        .await
        .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;
        if db_owner == "tankhor_fixture_runtime" {
            return Err(FixtureDatabaseError::PrivilegeSeparationViolated);
        }

        let flags: (bool, bool, bool, bool, bool) = sqlx::query_as(
            "SELECT rolsuper, rolcreatedb, rolcreaterole, rolreplication, rolbypassrls
             FROM pg_roles
             WHERE rolname = 'tankhor_fixture_runtime'",
        )
        .fetch_one(&self.admin_pool)
        .await
        .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;
        if flags != (false, false, false, false, false) {
            return Err(FixtureDatabaseError::PrivilegeSeparationViolated);
        }

        let mut runtime_conn = PgConnection::connect_with(&self.runtime_options())
            .await
            .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;
        let create_db_attempt = runtime_conn
            .execute("CREATE DATABASE forbidden_runtime_probe")
            .await;
        let set_role_attempt = runtime_conn
            .execute("SET ROLE tankhor_fixture_migration")
            .await;
        let _ = runtime_conn.close().await;

        if create_db_attempt.is_ok() || set_role_attempt.is_ok() {
            return Err(FixtureDatabaseError::PrivilegeSeparationViolated);
        }

        Ok(())
    }

    /// Verifies that an open application pool blocks plain `DROP DATABASE` (best-effort cleanup
    /// hazard), and that closing all application pools allows clean `DROP DATABASE` + absence.
    ///
    /// # Errors
    /// Returns [`FixtureDatabaseError::CleanupVerificationFailed`] if cleanup invariants fail.
    pub async fn verify_pool_cleanup_and_failed_retention_semantics(
        &self,
    ) -> Result<(), FixtureDatabaseError> {
        let probe_name = format!("fixture_cleanup_{}", uuid::Uuid::now_v7().simple());
        let server_opts = self.server_admin_options();
        let mut server_conn = PgConnection::connect_with(&server_opts)
            .await
            .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;

        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE DATABASE {probe_name} OWNER tankhor_fixture_migration"
        )))
        .execute(&mut server_conn)
        .await
        .map_err(|_| FixtureDatabaseError::CleanupVerificationFailed)?;

        let held_runtime_pool = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect_with(self.runtime_options().database(&probe_name))
            .await
            .map_err(|_| FixtureDatabaseError::CleanupVerificationFailed)?;
        let _: i32 = sqlx::query_scalar("SELECT 1")
            .fetch_one(&held_runtime_pool)
            .await
            .map_err(|_| FixtureDatabaseError::CleanupVerificationFailed)?;

        // With an active runtime connection open, plain DROP DATABASE fails and the DB remains.
        let drop_while_open =
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP DATABASE {probe_name}")))
                .execute(&mut server_conn)
                .await;
        if drop_while_open.is_ok() {
            let _ = server_conn.close().await;
            return Err(FixtureDatabaseError::CleanupVerificationFailed);
        }

        held_runtime_pool.close().await;

        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP DATABASE {probe_name}")))
            .execute(&mut server_conn)
            .await
            .map_err(|_| FixtureDatabaseError::CleanupVerificationFailed)?;
        server_conn
            .close()
            .await
            .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;

        verify_database_absent(&server_opts, &probe_name).await
    }

    pub async fn close(self) {
        self.admin_pool.close().await;
    }
}

/// Checks whether `database_name` exists in `pg_database` on the fixture server.
///
/// # Errors
/// Returns [`FixtureDatabaseError::ConnectionFailed`] if querying `pg_database` fails.
pub async fn database_exists_on_server(
    server_admin_options: &PgConnectOptions,
    database_name: &str,
) -> Result<bool, FixtureDatabaseError> {
    let mut conn =
        PgConnection::connect_with(&server_admin_options.clone().disable_statement_logging())
            .await
            .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(database_name)
            .fetch_one(&mut conn)
            .await
            .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;
    conn.close()
        .await
        .map_err(|_| FixtureDatabaseError::ConnectionFailed)?;
    Ok(exists)
}

/// Verifies that `database_name` has been dropped from `pg_database`.
///
/// # Errors
/// Returns [`FixtureDatabaseError::CleanupVerificationFailed`] if the database still exists.
pub async fn verify_database_absent(
    server_admin_options: &PgConnectOptions,
    database_name: &str,
) -> Result<(), FixtureDatabaseError> {
    if database_exists_on_server(server_admin_options, database_name).await? {
        return Err(FixtureDatabaseError::CleanupVerificationFailed);
    }
    Ok(())
}
