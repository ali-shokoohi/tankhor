//! In-memory deterministic fakes gated behind the `fakes` Cargo feature.

pub mod admin_authenticator;
pub mod admin_credential_repository;
pub mod admin_repository;
pub mod audit_repository;
pub mod change_repository;
pub mod clock;
pub mod flag_repository;
pub mod health_probe;
pub mod id_generator;
pub mod otp_challenge_repository;
pub mod rate_limit_repository;
pub mod session_store;
pub mod settings_repository;
pub mod sms_sender;
pub mod store;
pub mod unit_of_work;
