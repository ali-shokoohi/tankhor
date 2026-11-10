//! Real PostgreSQL storage adapters and canonical migration registry.

pub mod unit_of_work;

/// Canonical workspace `SQLx` migrator rooted at `crates/infra-postgres/migrations`.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
