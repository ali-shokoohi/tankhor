//! Real PostgreSQL storage adapters and canonical migration registry.

pub mod admin_credential_repository;
pub mod admin_repository;
pub mod audit_repository;
mod error;
pub mod otp_challenge_repository;
pub mod rate_limit_repository;
pub mod session_store;
pub mod unit_of_work;

/// Canonical workspace `SQLx` migrator rooted at `crates/infra-postgres/migrations`.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
