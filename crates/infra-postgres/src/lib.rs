//! Real PostgreSQL storage adapters and canonical migration registry.

pub mod admin_credential_repository;
pub mod admin_repository;
pub mod audit_repository;
pub mod change_repository;
mod error;
pub mod flag_repository;
pub mod health_probe;
pub mod otp_challenge_repository;
pub mod rate_limit_repository;
pub mod session_store;
pub mod settings_repository;
pub mod unit_of_work;

/// Canonical workspace `SQLx` migrator rooted at `crates/infra-postgres/migrations`.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Applies canonical workspace migrations using the privileged migration database URL.
///
/// # Errors
/// Returns a sanitized [`DomainError`] if connecting or running migrations fails, without
/// exposing the database URL or credentials.
pub async fn run_migrations(database_url: &str) -> Result<(), kernel::error::DomainError> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .map_err(|_| error::unavailable())?;
    let result = MIGRATOR.run(&pool).await.map_err(|_| error::unavailable());
    pool.close().await;
    result
}
