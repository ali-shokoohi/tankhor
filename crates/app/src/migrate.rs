//! Protected operator command for applying canonical workspace database migrations.
//!
//! Privileged migration credentials are read only from protected environment configuration
//! (`TANKHOR_MIGRATION_DATABASE_URL`, falling back to `DATABASE_URL`) and are never accepted
//! via CLI arguments or emitted in error messages.

use crate::config::{AdapterMode, AppConfig};

/// Environment variable specifying the privileged migration database URL.
pub const MIGRATION_DATABASE_URL_ENV: &str = "TANKHOR_MIGRATION_DATABASE_URL";

/// Sanitized errors returned by `app migrate`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MigrateError {
    #[error("usage: app migrate (database URL from protected environment)")]
    InvalidCommand,
    #[error("invalid application configuration")]
    Configuration,
    #[error("migration database URL is missing from protected configuration")]
    MissingDatabaseUrl,
    #[error(
        "TANKHOR_ADAPTERS=fake requires debug feature `dev-fakes` and is unavailable in default/release builds"
    )]
    FakeAdaptersNotCompiled,
    #[error("database migration failed")]
    MigrationFailed,
}

/// Validates that no CLI arguments (such as secret-bearing database URLs) were passed to `migrate`.
///
/// # Errors
/// Rejects any argument without echoing its value.
pub fn validate_migrate_command<I, S>(args: I) -> Result<(), MigrateError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    if args.into_iter().next().is_some() {
        return Err(MigrateError::InvalidCommand);
    }
    Ok(())
}

/// Executes canonical workspace migrations according to [`AppConfig`].
///
/// In `AdapterMode::Fake` (with `dev-fakes`), performs a no-op without contacting external storage.
/// In `AdapterMode::Real`, applies `infra_postgres::MIGRATOR` using the privileged migration URL.
///
/// # Errors
/// Returns a sanitized [`MigrateError`] if configuration or migration execution fails.
pub async fn run_migrate(
    config: &AppConfig,
    database_url: Option<&str>,
) -> Result<(), MigrateError> {
    match config.adapter_mode() {
        AdapterMode::Fake => run_fake_migrate(),
        AdapterMode::Real => {
            let url = database_url
                .map(str::trim)
                .filter(|val| !val.is_empty())
                .ok_or(MigrateError::MissingDatabaseUrl)?;
            infra_postgres::run_migrations(url)
                .await
                .map_err(|_| MigrateError::MigrationFailed)
        }
    }
}

#[cfg(feature = "dev-fakes")]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Fake-enabled and fake-disabled builds share the same fallible signature"
)]
const fn run_fake_migrate() -> Result<(), MigrateError> {
    Ok(())
}

#[cfg(not(feature = "dev-fakes"))]
const fn run_fake_migrate() -> Result<(), MigrateError> {
    Err(MigrateError::FakeAdaptersNotCompiled)
}

/// Validates CLI arguments and environment configuration, then executes database migrations.
///
/// # Errors
/// Returns a sanitized [`MigrateError`] on invalid arguments, missing configuration, or SQL failure.
pub async fn migrate_from_env<I, S>(args: I) -> Result<(), MigrateError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    validate_migrate_command(args)?;
    let config = AppConfig::from_env().map_err(|_| MigrateError::Configuration)?;
    let url = std::env::var(MIGRATION_DATABASE_URL_ENV)
        .ok()
        .filter(|val| !val.trim().is_empty())
        .or_else(|| {
            std::env::var("DATABASE_URL")
                .ok()
                .filter(|val| !val.trim().is_empty())
        });
    run_migrate(&config, url.as_deref()).await
}
