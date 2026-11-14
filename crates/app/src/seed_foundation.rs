//! Protected operator command for idempotent foundational registry seeding.
//!
//! Executes [`SeedFoundation`] through the application-owned `UnitOfWork` using restricted
//! runtime credentials (never privileged migration/superuser credentials) and never accepts
//! or logs deployment secrets via CLI arguments.

use admin::application::seed_foundation::{SeedFoundation, SeedFoundationInput, SeedOutcome};
use kernel::error::ErrorCode;
use kernel::ids::{AdminId, AuditId};

use crate::composition::{RegistryComposition, compose_registry};
use crate::config::AppConfig;

/// Parsed non-secret arguments for `app seed-foundation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedFoundationCommand {
    system_id: AdminId,
    operation_id: AuditId,
}

impl SeedFoundationCommand {
    /// Parses only the non-secret system actor identity and operation audit ID.
    ///
    /// # Errors
    /// Rejects missing, duplicate, malformed, or unknown arguments without echoing their values.
    pub fn parse<I, S>(args: I) -> Result<Self, SeedFoundationError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let mut system_id = None;
        let mut operation_id = None;
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--system-id" if system_id.is_none() => {
                    let value = args.next().ok_or(SeedFoundationError::InvalidCommand)?;
                    system_id = Some(
                        value
                            .parse()
                            .map_err(|_| SeedFoundationError::InvalidCommand)?,
                    );
                }
                "--operation-id" if operation_id.is_none() => {
                    let value = args.next().ok_or(SeedFoundationError::InvalidCommand)?;
                    operation_id = Some(
                        value
                            .parse()
                            .map_err(|_| SeedFoundationError::InvalidCommand)?,
                    );
                }
                _ => return Err(SeedFoundationError::InvalidCommand),
            }
        }
        Ok(Self {
            system_id: system_id.ok_or(SeedFoundationError::InvalidCommand)?,
            operation_id: operation_id.ok_or(SeedFoundationError::InvalidCommand)?,
        })
    }
}

/// Sanitized errors returned by `app seed-foundation`.
#[derive(Debug, thiserror::Error)]
pub enum SeedFoundationError {
    #[error(
        "usage: app seed-foundation --system-id <UUID> --operation-id <UUID> (database URL from protected environment)"
    )]
    InvalidCommand,
    #[error("invalid application configuration")]
    Configuration,
    #[error("registry adapter composition failed")]
    Composition,
    #[error("seed-foundation requires restricted runtime database credentials")]
    PrivilegeViolation,
    #[error("foundation registry seeding was refused: {0}")]
    Seeding(#[from] kernel::error::DomainError),
}

/// Executes [`SeedFoundation`] over the composed registry ports after verifying restricted
/// runtime credentials when running against PostgreSQL.
///
/// # Errors
/// Returns [`SeedFoundationError::PrivilegeViolation`] if connected as a migration owner or
/// superuser, or propagates application service errors.
pub async fn run_seed_foundation(
    command: SeedFoundationCommand,
    registry: &RegistryComposition,
) -> Result<SeedOutcome, SeedFoundationError> {
    if let Err(err) = registry.verify_runtime_role().await {
        return if err.code() == ErrorCode::PermissionDenied {
            Err(SeedFoundationError::PrivilegeViolation)
        } else {
            Err(SeedFoundationError::Seeding(err))
        };
    }

    let uow = registry.uow();
    let settings = registry.settings();
    let flags = registry.flags();
    let audit = registry.audit();
    let clock = registry.clock();
    let ids = registry.ids();

    (SeedFoundation {
        uow: uow.as_ref(),
        settings: settings.as_ref(),
        flags: flags.as_ref(),
        audit: audit.as_ref(),
        clock: clock.as_ref(),
        ids: ids.as_ref(),
    })
    .execute(SeedFoundationInput {
        operation: command.operation_id,
        system: command.system_id,
    })
    .await
    .map_err(SeedFoundationError::from)
}

/// Validates CLI arguments and environment configuration, composes the registry backend, and
/// executes [`SeedFoundation`].
///
/// # Errors
/// Returns sanitized configuration, composition, privilege, or seeding errors.
pub async fn seed_foundation_from_env<I, S>(args: I) -> Result<SeedOutcome, SeedFoundationError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let command = SeedFoundationCommand::parse(args)?;
    let config = AppConfig::from_env().map_err(|_| SeedFoundationError::Configuration)?;
    let registry = compose_registry(&config).map_err(|_| SeedFoundationError::Composition)?;
    let result = run_seed_foundation(command, &registry).await;
    registry.close().await;
    result
}
