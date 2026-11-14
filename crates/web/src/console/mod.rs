//! Administrator console HTML delivery, calling only application services.

use std::sync::Arc;

pub use admin::application::console_services::ConsoleServices;

pub mod admins;
pub mod auth;
pub mod changes;
pub mod enrollment;
pub mod guard;
pub mod settings;

/// Assemble only delivered US1 routes over one application-service port collection.
pub fn access_router(
    services: ConsoleServices,
    catalog: crate::i18n::ConsoleCatalog,
    csrf: crate::security::CsrfProtectionState,
    cookies: crate::security::CookieSecurityPolicy,
) -> axum::Router {
    auth::sign_in_router(Arc::clone(&services.authenticator), catalog, cookies)
        .merge(auth::passkey_bridge_router())
        .merge(auth::request_code_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(auth::verify_code_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(auth::second_factor_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(auth::passkey_begin_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(auth::passkey_finish_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(auth::totp_router(services.clone(), csrf.clone(), cookies))
        .merge(auth::sign_out_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(admins::admin_list_router(services.clone(), cookies))
        .merge(admins::admin_invite_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(admins::admin_recover_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(enrollment::enrollment_router(services.clone(), cookies))
        .merge(enrollment::enroll_passkey_begin_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(enrollment::enroll_passkey_finish_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(enrollment::enroll_totp_begin_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(enrollment::enroll_totp_finish_router(
            services, csrf, cookies,
        ))
}

/// Assemble delivered US2 settings and change-preview routes.
pub fn registry_router(
    services: ConsoleServices,
    csrf: crate::security::CsrfProtectionState,
    cookies: crate::security::CookieSecurityPolicy,
) -> axum::Router {
    settings::settings_list_router(services.clone(), cookies)
        .merge(settings::setting_detail_router(services.clone(), cookies))
        .merge(settings::setting_draft_router(
            services.clone(),
            csrf.clone(),
            cookies,
        ))
        .merge(changes::change_detail_router(services.clone(), cookies))
        .merge(changes::change_preview_router(services, csrf, cookies))
}
