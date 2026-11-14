//! Read-only PostgreSQL readiness probe verifying canonical migrated and seeded registries.

use admin::ports::health_probe::{HealthProbe, ProbeReadiness};
use kernel::error::DomainError;
use sqlx::PgPool;

use crate::error::unavailable;

#[derive(Clone, Debug)]
pub struct PostgresHealthProbe {
    pool: PgPool,
}

impl PostgresHealthProbe {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl HealthProbe for PostgresHealthProbe {
    async fn check(&self) -> Result<ProbeReadiness, DomainError> {
        let mut connection = self.pool.acquire().await.map_err(|_| unavailable())?;
        let settings_seeded = match sqlx::query!(
            "SELECT COUNT(*) AS count FROM settings s
             JOIN setting_versions v ON v.id = s.current_version_id AND v.setting_key = s.key"
        )
        .fetch_one(&mut *connection)
        .await
        {
            Ok(row) => row.count.unwrap_or(0) == 10,
            Err(error) => {
                return if is_missing_relation(&error) {
                    Ok(ProbeReadiness::Unseeded)
                } else {
                    Err(unavailable())
                };
            }
        };
        let flags_seeded = match sqlx::query!(
            "SELECT COUNT(*) AS count FROM flags f
             JOIN flag_versions v ON v.id = f.current_version_id AND v.flag_key = f.key"
        )
        .fetch_one(&mut *connection)
        .await
        {
            Ok(row) => row.count.unwrap_or(0) == 6,
            Err(error) => {
                return if is_missing_relation(&error) {
                    Ok(ProbeReadiness::Unseeded)
                } else {
                    Err(unavailable())
                };
            }
        };
        if settings_seeded && flags_seeded {
            Ok(ProbeReadiness::Ready)
        } else {
            Ok(ProbeReadiness::Unseeded)
        }
    }
}

fn is_missing_relation(error: &sqlx::Error) -> bool {
    matches!(
        error
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("42P01")
    )
}
