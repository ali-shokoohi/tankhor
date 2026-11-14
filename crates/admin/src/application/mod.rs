//! Admin application services and transactional orchestration helpers.

pub mod bootstrap_admin;
pub mod console_access;
pub mod console_services;
pub mod enrollment;
pub mod get_change;
pub mod get_enrollment;
pub mod get_setting;
pub mod get_version;
pub mod health;
pub mod invite_admin;
pub mod list_admins;
pub mod list_flags;
pub mod list_settings;
pub mod passkey;
pub mod prepare_access_form;
pub mod preview;
pub mod read_transaction;
pub mod recover_admin;
pub mod request_admin_otp;
pub mod save_draft;
pub mod seed_foundation;
pub mod sign_out;
pub mod totp;
pub mod verify_admin_otp;
