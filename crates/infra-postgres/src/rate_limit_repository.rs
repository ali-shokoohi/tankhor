//! Purpose-separated, atomically incremented finite abuse-budget windows.

use admin::domain::security_policy::RateLimit;
use admin::ports::rate_limit_repository::{
    RateLimitKey, RateLimitPurpose, RateLimitRepository, RateLimitScope, RateLimitStatus,
};
use kernel::error::{DomainError, ErrorCode};
use kernel::ports::unit_of_work::TxHandle;
use time::OffsetDateTime;

use crate::error::{map_sql, stale};
use crate::unit_of_work::PostgresUnitOfWork;

#[derive(Clone, Debug)]
pub struct PostgresRateLimitRepository {
    uow: PostgresUnitOfWork,
}

impl PostgresRateLimitRepository {
    #[must_use]
    pub const fn new(uow: PostgresUnitOfWork) -> Self {
        Self { uow }
    }
}

#[async_trait::async_trait]
impl RateLimitRepository for PostgresRateLimitRepository {
    async fn increment(
        &self,
        tx: &mut TxHandle,
        key: RateLimitKey,
        limit: RateLimit,
        now: OffsetDateTime,
    ) -> Result<RateLimitStatus, DomainError> {
        let connection = self.uow.writable_connection(tx)?;
        let maximum = i64::try_from(limit.max_attempts().get())
            .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
        let window = time::SignedDuration::try_from(limit.window())
            .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
        let expires = now
            .checked_add(window)
            .ok_or_else(|| DomainError::validation(ErrorCode::InvalidValue))?;
        let digest = key.digest();
        for _ in 0..16 {
            let existing = load_latest(connection, key).await?;
            if let Some(row) = existing {
                if now < row.window_started_at {
                    return Err(DomainError::validation(ErrorCode::InvalidValue));
                }
                if now < row.window_expires_at {
                    if row.max_attempts != maximum || row.count == i64::MAX {
                        return Err(DomainError::validation(ErrorCode::InvalidValue));
                    }
                    let updated = sqlx::query_as!(
                        RateLimitRow,
                        "UPDATE admin_rate_limits SET count = count + 1
                         WHERE scope = $1 AND purpose = $2 AND bucket_digest = $3
                           AND window_started_at = $4 AND count < 9223372036854775807
                         RETURNING window_started_at, window_expires_at,
                                   count, max_attempts, blocked_until",
                        scope_text(key.scope()),
                        purpose_text(key.purpose()),
                        digest.as_slice(),
                        row.window_started_at,
                    )
                    .fetch_one(&mut *connection)
                    .await
                    .map_err(|error| map_sql(&error))?;
                    return updated.into_status();
                }
            }
            let inserted = sqlx::query!(
                "INSERT INTO admin_rate_limits (
                     scope, purpose, bucket_digest, window_started_at, window_expires_at,
                     count, max_attempts, blocked_until
                 ) VALUES ($1, $2, $3, $4, $5, 1, $6, NULL)
                 ON CONFLICT DO NOTHING",
                scope_text(key.scope()),
                purpose_text(key.purpose()),
                digest.as_slice(),
                now,
                expires,
                maximum,
            )
            .execute(&mut *connection)
            .await
            .map_err(|error| map_sql(&error))?;
            if inserted.rows_affected() == 1 {
                return Ok(RateLimitStatus {
                    count: 1,
                    started_at: now,
                    expires_at: expires,
                    blocked_until: None,
                    allowed: true,
                });
            }
        }
        Err(DomainError::retryable(ErrorCode::TransientConflict))
    }

    async fn read(
        &self,
        tx: &mut TxHandle,
        key: RateLimitKey,
        now: OffsetDateTime,
    ) -> Result<Option<RateLimitStatus>, DomainError> {
        let connection = self.uow.readable_connection(tx)?;
        let digest = key.digest();
        let Some(row) = sqlx::query_as!(
            RateLimitRow,
            "SELECT window_started_at, window_expires_at, count, max_attempts, blocked_until
             FROM admin_rate_limits
             WHERE scope = $1 AND purpose = $2 AND bucket_digest = $3
             ORDER BY window_started_at DESC LIMIT 1",
            scope_text(key.scope()),
            purpose_text(key.purpose()),
            digest.as_slice(),
        )
        .fetch_optional(connection)
        .await
        .map_err(|error| map_sql(&error))?
        else {
            return Ok(None);
        };
        if now < row.window_started_at {
            return Err(DomainError::validation(ErrorCode::InvalidValue));
        }
        if now >= row.window_expires_at {
            return Ok(None);
        }
        row.into_status().map(Some)
    }
}

struct RateLimitRow {
    window_started_at: OffsetDateTime,
    window_expires_at: OffsetDateTime,
    count: i64,
    max_attempts: i64,
    blocked_until: Option<OffsetDateTime>,
}

impl RateLimitRow {
    fn into_status(self) -> Result<RateLimitStatus, DomainError> {
        let count = u64::try_from(self.count).map_err(|_| stale())?;
        let maximum = u64::try_from(self.max_attempts).map_err(|_| stale())?;
        let allowed = count <= maximum;
        Ok(RateLimitStatus {
            count,
            started_at: self.window_started_at,
            expires_at: self.window_expires_at,
            blocked_until: if allowed {
                self.blocked_until
            } else {
                Some(self.window_expires_at)
            },
            allowed,
        })
    }
}

async fn load_latest(
    connection: &mut sqlx::PgConnection,
    key: RateLimitKey,
) -> Result<Option<RateLimitRow>, DomainError> {
    let digest = key.digest();
    sqlx::query_as!(
        RateLimitRow,
        "SELECT window_started_at, window_expires_at, count, max_attempts, blocked_until
         FROM admin_rate_limits
         WHERE scope = $1 AND purpose = $2 AND bucket_digest = $3
         ORDER BY window_started_at DESC LIMIT 1 FOR UPDATE",
        scope_text(key.scope()),
        purpose_text(key.purpose()),
        digest.as_slice(),
    )
    .fetch_optional(connection)
    .await
    .map_err(|error| map_sql(&error))
}

const fn scope_text(scope: RateLimitScope) -> &'static str {
    match scope {
        RateLimitScope::Subject => "subject",
        RateLimitScope::Origin => "origin",
    }
}

const fn purpose_text(purpose: RateLimitPurpose) -> &'static str {
    match purpose {
        RateLimitPurpose::Send => "send",
        RateLimitPurpose::Verify => "verify",
    }
}
