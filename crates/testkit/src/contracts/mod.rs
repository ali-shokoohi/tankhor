//! Shared generic contract suites run unchanged against fakes and real adapters.

pub mod admin_credential_repository;
pub mod admin_repository;
pub mod audit_repository;
pub mod clock;
pub mod health_probe;
pub mod id_generator;
pub mod otp_challenge_repository;
pub mod session_store;
pub mod unit_of_work;
