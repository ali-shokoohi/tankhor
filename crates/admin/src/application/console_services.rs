//! Shared outbound ports for constructing administrator application-service calls.

use std::sync::Arc;

use crate::{
    domain::security_policy::SecurityPolicy,
    ports::{
        admin_authenticator::AdminAuthenticator,
        admin_credential_repository::AdminCredentialRepository, admin_repository::AdminRepository,
        audit_repository::AuditRepository, change_repository::ChangeRepository,
        flag_repository::FlagRepository, otp_challenge_repository::OtpChallengeRepository,
        rate_limit_repository::RateLimitRepository, session_store::SessionStore,
        settings_repository::SettingsRepository, sms_sender::SmsSender,
    },
};
use kernel::ports::{clock::Clock, id_generator::IdGenerator, unit_of_work::UnitOfWork};

#[derive(Clone)]
pub struct ConsoleServices {
    pub uow: Arc<dyn UnitOfWork>,
    pub admins: Arc<dyn AdminRepository>,
    pub credentials: Arc<dyn AdminCredentialRepository>,
    pub sessions: Arc<dyn SessionStore>,
    pub otps: Arc<dyn OtpChallengeRepository>,
    pub rates: Arc<dyn RateLimitRepository>,
    pub audit: Arc<dyn AuditRepository>,
    pub settings: Arc<dyn SettingsRepository>,
    pub flags: Arc<dyn FlagRepository>,
    pub changes: Arc<dyn ChangeRepository>,
    pub authenticator: Arc<dyn AdminAuthenticator>,
    pub primary_sms: Arc<dyn SmsSender>,
    pub secondary_sms: Arc<dyn SmsSender>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdGenerator>,
    pub policy: Arc<SecurityPolicy>,
}
