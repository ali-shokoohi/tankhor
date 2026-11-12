//! Protected operator command for the one-time initial administrator ceremony.
//!
//! The phone secret is read only from protected process configuration, never a CLI
//! argument or output. All eligibility, transaction and audit policy remains in
//! [`BootstrapAdmin`].

use admin::{
    application::bootstrap_admin::{BootstrapAdmin, BootstrapAdminInput, OperatorContext},
    domain::secrets::SecretBytes,
};
use kernel::ids::AdminId;
use web::console::ConsoleServices;

use crate::{composition::compose_access, config::AppConfig};

pub const BOOTSTRAP_PHONE_ENV: &str = "TANKHOR_BOOTSTRAP_PHONE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvisionAdminCommand {
    operator_id: AdminId,
}

impl ProvisionAdminCommand {
    /// Parse only the non-secret operator identity and explicit owner authorization.
    ///
    /// # Errors
    /// Rejects missing/duplicate authorization, malformed IDs and every unknown option.
    /// Error text deliberately never echoes supplied arguments.
    pub fn parse<I, S>(args: I) -> Result<Self, ProvisionAdminError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let mut operator_id = None;
        let mut owner_authorized = false;
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--operator-id" if operator_id.is_none() => {
                    let value = args.next().ok_or(ProvisionAdminError::InvalidCommand)?;
                    operator_id = Some(
                        value
                            .parse()
                            .map_err(|_| ProvisionAdminError::InvalidCommand)?,
                    );
                }
                "--owner-authorized" if !owner_authorized => owner_authorized = true,
                _ => return Err(ProvisionAdminError::InvalidCommand),
            }
        }
        if !owner_authorized {
            return Err(ProvisionAdminError::InvalidCommand);
        }
        Ok(Self {
            operator_id: operator_id.ok_or(ProvisionAdminError::InvalidCommand)?,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProvisionAdminError {
    #[error(
        "usage: app provision-admin --operator-id <UUID> --owner-authorized (phone from protected environment)"
    )]
    InvalidCommand,
    #[error("initial administrator phone is missing from protected configuration")]
    MissingPhone,
    #[error("invalid application configuration")]
    Configuration,
    #[error("administrator adapter composition failed")]
    Composition,
    #[error("initial administrator provisioning was refused: {0}")]
    Provisioning(#[from] kernel::error::DomainError),
}

/// Delegate one provisioning attempt to the completed application service.
///
/// # Errors
/// Returns the service's safe error without exposing the phone or bypassing its policy.
pub async fn run_provision_admin(
    command: ProvisionAdminCommand,
    services: ConsoleServices,
    phone: SecretBytes,
) -> Result<AdminId, ProvisionAdminError> {
    (BootstrapAdmin {
        uow: services.uow.as_ref(),
        admins: services.admins.as_ref(),
        audit: services.audit.as_ref(),
        authenticator: services.authenticator.as_ref(),
        clock: services.clock.as_ref(),
        ids: services.ids.as_ref(),
        policy: &services.policy,
    })
    .execute(BootstrapAdminInput {
        operator: OperatorContext::new(command.operator_id, true),
        phone,
    })
    .await
    .map_err(ProvisionAdminError::from)
}

/// Compose the configured single backend group and execute the protected command.
///
/// # Errors
/// Returns sanitized configuration, composition or application-service errors.
pub async fn provision_from_env<I, S>(args: I) -> Result<AdminId, ProvisionAdminError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let command = ProvisionAdminCommand::parse(args)?;
    let phone =
        std::env::var(BOOTSTRAP_PHONE_ENV).map_err(|_| ProvisionAdminError::MissingPhone)?;
    let config = AppConfig::from_env().map_err(|_| ProvisionAdminError::Configuration)?;
    let composition = compose_access(&config).map_err(|_| ProvisionAdminError::Composition)?;
    run_provision_admin(
        command,
        composition.services(),
        SecretBytes::new(phone.into_bytes()),
    )
    .await
}
