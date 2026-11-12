//! Protected enrollment choices and ceremonies; application services own every authorization decision.

use admin::{
    application::{
        enrollment::{
            BeginPasskeyEnrollment, BeginPasskeyEnrollmentInput, BeginTotpEnrollment,
            BeginTotpEnrollmentInput, FinishPasskeyEnrollment, FinishPasskeyEnrollmentInput,
            FinishTotpEnrollment, FinishTotpEnrollmentInput,
        },
        get_enrollment::{EnrollmentFactor, GetEnrollment, GetEnrollmentInput},
        verify_admin_otp::PrimaryProof,
    },
    domain::{
        admin_state::{EnrollmentPurpose, SecurityEpoch},
        auth_state::{AuthBinding, AuthPurpose, AuthStage},
        secrets::SecretBytes,
    },
    ports::{admin_authenticator::TokenPurpose, session_store::PendingCheck},
};
use askama::Template;
use axum::{
    Router,
    extract::{Extension, Form, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use kernel::{
    error::ErrorCode,
    ids::{AdminId, SessionId},
    locale::Locale,
};
use serde::Deserialize;

use super::{
    ConsoleServices,
    auth::{cookie, cookie_value},
};
use crate::{
    i18n::ConsoleCatalog,
    security::{
        CookieKind, CookieSecurityPolicy, CsrfContext, CsrfProtectionState,
        enforce_csrf_middleware, format_cookie_header,
    },
};

#[derive(Debug, Deserialize)]
struct EnrollmentQuery {
    target: AdminId,
    session: SessionId,
    epoch: u64,
    purpose: String,
    lang: Option<String>,
}

#[derive(Template)]
#[template(path = "console/enrollment.html")]
struct EnrollmentView {
    lang: &'static str,
    dir: &'static str,
    heading: String,
    guidance: String,
    passkey: String,
    totp: String,
    target: AdminId,
    session: SessionId,
    epoch: u64,
    purpose: String,
    passkey_available: bool,
    totp_available: bool,
    csrf: Option<String>,
}

/// Show only application-authorized enrollment choices.
pub fn enrollment_router(services: ConsoleServices, cookies: CookieSecurityPolicy) -> Router {
    Router::new()
        .route("/console/access/enrol", get(handle_enrollment))
        .with_state((services, cookies))
}

async fn handle_enrollment(
    State((services, cookies)): State<(ConsoleServices, CookieSecurityPolicy)>,
    headers: HeaderMap,
    csrf: Option<Extension<CsrfContext>>,
    Query(query): Query<EnrollmentQuery>,
) -> Response {
    let locale = query
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    let purpose = match query.purpose.as_str() {
        "addition" => EnrollmentPurpose::Addition,
        "recovery" => EnrollmentPurpose::Recovery,
        _ => return enrollment_error(locale, StatusCode::UNPROCESSABLE_ENTITY),
    };
    let result = async {
        let proof =
            cookie(&headers, CookieKind::PrimaryProof.cookie_name(cookies)).ok_or_else(|| {
                kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
            })?;
        let browser = cookie(&headers, CookieKind::PrimaryCeremony.cookie_name(cookies))
            .ok_or_else(|| {
                kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
            })?;
        let browser = SecretBytes::new(browser.as_bytes().to_vec());
        let browser_digest = services
            .authenticator
            .token_digest(TokenPurpose::Browser, browser.clone())
            .await?;
        (GetEnrollment {
            uow: services.uow.as_ref(),
            admins: services.admins.as_ref(),
            sessions: services.sessions.as_ref(),
            authenticator: services.authenticator.as_ref(),
            clock: services.clock.as_ref(),
        })
        .execute(GetEnrollmentInput {
            proof: PrimaryProof {
                token: SecretBytes::new(proof.as_bytes().to_vec()),
                binding: AuthBinding {
                    target: query.target,
                    browser: browser_digest,
                    session: query.session,
                    purpose: AuthPurpose::Enrollment(purpose),
                    stage: AuthStage::PrimaryVerified,
                    epoch: SecurityEpoch::new(query.epoch),
                    credential_revision: None,
                },
            },
            browser,
        })
        .await
    }
    .await;
    match result {
        Ok(view) => {
            let catalog = ConsoleCatalog::load_authoritative();
            let Ok(catalog) = catalog else {
                return enrollment_error(locale, StatusCode::INTERNAL_SERVER_ERROR);
            };
            let text = |key| {
                catalog
                    .format(locale, key, &[])
                    .unwrap_or_else(|_| key.to_owned())
            };
            let template = EnrollmentView {
                lang: locale.as_tag(),
                dir: locale.direction().as_str(),
                heading: text("enrollment-heading"),
                guidance: text("passkey-disabled-guidance"),
                passkey: text("enrollment-passkey"),
                totp: text("enrollment-totp"),
                target: view.target,
                session: query.session,
                epoch: query.epoch,
                purpose: query.purpose.clone(),
                passkey_available: view.factors.contains(&EnrollmentFactor::Passkey),
                totp_available: view.factors.contains(&EnrollmentFactor::Totp),
                csrf: csrf.and_then(|Extension(context)| {
                    std::str::from_utf8(context.expected_bytes())
                        .ok()
                        .map(str::to_owned)
                }),
            };
            match template.render() {
                Ok(html) => (
                    StatusCode::OK,
                    [(header::CACHE_CONTROL, "no-store")],
                    Html(html),
                )
                    .into_response(),
                Err(_) => enrollment_error(locale, StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            enrollment_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => enrollment_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct ProofForm {
    target: AdminId,
    session: SessionId,
    epoch: u64,
    purpose: String,
    csrf_token: String,
    lang: Option<String>,
}

#[derive(Template)]
#[template(path = "console/enrollment_passkey.html")]
struct EnrollmentPasskeyView {
    lang: &'static str,
    dir: &'static str,
    heading: String,
    public_options: String,
    target: AdminId,
    session: SessionId,
    epoch: u64,
    purpose: String,
    credential: kernel::ids::CredentialId,
    csrf: String,
}

/// Begin passkey enrollment and expose only public `WebAuthn` options.
pub fn enroll_passkey_begin_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/enrol/passkey/begin", post(handle_passkey_begin))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state((services, cookies))
}

async fn handle_passkey_begin(
    State((services, cookies)): State<(ConsoleServices, CookieSecurityPolicy)>,
    headers: HeaderMap,
    csrf: Option<Extension<CsrfContext>>,
    Form(form): Form<ProofForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return enrollment_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let proof = primary_pending_check(&services, &headers, cookies, &form).await?;
        (BeginPasskeyEnrollment {
            uow: services.uow.as_ref(),
            admins: services.admins.as_ref(),
            sessions: services.sessions.as_ref(),
            audit: services.audit.as_ref(),
            authenticator: services.authenticator.as_ref(),
            clock: services.clock.as_ref(),
            ids: services.ids.as_ref(),
            policy: &services.policy,
        })
        .execute(BeginPasskeyEnrollmentInput { proof })
        .await
    }
    .await;
    match result {
        Ok(options) => {
            let Ok(token) = cookie_value(&options.token) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(cookie) = format_cookie_header(CookieKind::PasskeyCeremony, &token, cookies)
            else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(public_options) = std::str::from_utf8(options.public_options.expose()) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            let catalog = ConsoleCatalog::load_authoritative();
            let Ok(catalog) = catalog else {
                return enrollment_error(locale, StatusCode::INTERNAL_SERVER_ERROR);
            };
            let template = EnrollmentPasskeyView {
                lang: locale.as_tag(),
                dir: locale.direction().as_str(),
                heading: catalog
                    .format(locale, "passkey-heading", &[])
                    .unwrap_or_else(|_| "Passkey".to_owned()),
                public_options: public_options.to_owned(),
                target: options.binding.target,
                session: options.binding.session,
                epoch: options.binding.epoch.value(),
                purpose: form.purpose,
                credential: options.credential,
                csrf: csrf
                    .and_then(|Extension(context)| {
                        std::str::from_utf8(context.expected_bytes())
                            .ok()
                            .map(str::to_owned)
                    })
                    .unwrap_or_default(),
            };
            match template.render() {
                Ok(html) => (
                    StatusCode::OK,
                    [
                        (header::CACHE_CONTROL, "no-store"),
                        (header::SET_COOKIE, cookie.as_str()),
                    ],
                    Html(html),
                )
                    .into_response(),
                Err(_) => enrollment_error(locale, StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            enrollment_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => enrollment_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Template)]
#[template(path = "console/totp_provisioning.html")]
struct TotpProvisioningView {
    lang: &'static str,
    dir: &'static str,
    heading: String,
    provisioning: String,
    target: AdminId,
    session: SessionId,
    epoch: u64,
    purpose: String,
    credential: kernel::ids::CredentialId,
    csrf: String,
}

/// Present TOTP provisioning exactly once in the direct authenticated response.
pub fn enroll_totp_begin_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route(
            "/console/enrol/totp/begin",
            post(handle_totp_begin).get(totp_begin_get_refused),
        )
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state((services, cookies))
}

async fn totp_begin_get_refused() -> Response {
    enrollment_error(Locale::En, StatusCode::METHOD_NOT_ALLOWED)
}

async fn handle_totp_begin(
    State((services, cookies)): State<(ConsoleServices, CookieSecurityPolicy)>,
    headers: HeaderMap,
    csrf: Option<Extension<CsrfContext>>,
    Form(form): Form<ProofForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return enrollment_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let proof = primary_pending_check(&services, &headers, cookies, &form).await?;
        (BeginTotpEnrollment {
            uow: services.uow.as_ref(),
            admins: services.admins.as_ref(),
            sessions: services.sessions.as_ref(),
            audit: services.audit.as_ref(),
            authenticator: services.authenticator.as_ref(),
            clock: services.clock.as_ref(),
            ids: services.ids.as_ref(),
            policy: &services.policy,
        })
        .execute(BeginTotpEnrollmentInput {
            proof,
            predecessor: None,
        })
        .await
    }
    .await;
    match result {
        Ok(presentation) => {
            let Ok(token) = cookie_value(&presentation.token) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(cookie) = format_cookie_header(CookieKind::TotpCeremony, &token, cookies) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(provisioning) = std::str::from_utf8(presentation.provisioning.expose()) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            let catalog = ConsoleCatalog::load_authoritative();
            let Ok(catalog) = catalog else {
                return enrollment_error(locale, StatusCode::INTERNAL_SERVER_ERROR);
            };
            let template = TotpProvisioningView {
                lang: locale.as_tag(),
                dir: locale.direction().as_str(),
                heading: catalog
                    .format(locale, "enrollment-totp", &[])
                    .unwrap_or_else(|_| "Authenticator".to_owned()),
                provisioning: provisioning.to_owned(),
                target: presentation.binding.target,
                session: presentation.binding.session,
                epoch: presentation.binding.epoch.value(),
                purpose: form.purpose,
                credential: presentation.credential,
                csrf: csrf
                    .and_then(|Extension(context)| {
                        std::str::from_utf8(context.expected_bytes())
                            .ok()
                            .map(str::to_owned)
                    })
                    .unwrap_or_default(),
            };
            match template.render() {
                Ok(html) => (
                    StatusCode::OK,
                    [
                        (header::CACHE_CONTROL, "no-store"),
                        (header::SET_COOKIE, cookie.as_str()),
                    ],
                    Html(html),
                )
                    .into_response(),
                Err(_) => enrollment_error(locale, StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            enrollment_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => enrollment_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct FinishPasskeyForm {
    credential: kernel::ids::CredentialId,
    target: AdminId,
    session: SessionId,
    epoch: u64,
    credential_revision: u64,
    purpose: String,
    response: String,
    csrf_token: String,
    lang: Option<String>,
}

/// Finish passkey enrollment through the transactional application service.
pub fn enroll_passkey_finish_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/enrol/passkey/finish", post(handle_passkey_finish))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state((services, cookies))
}

async fn handle_passkey_finish(
    State((services, cookies)): State<(ConsoleServices, CookieSecurityPolicy)>,
    headers: HeaderMap,
    Form(form): Form<FinishPasskeyForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return enrollment_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let ceremony = cookie(&headers, CookieKind::PasskeyCeremony.cookie_name(cookies))
            .ok_or_else(|| {
                kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
            })?;
        let browser = cookie(&headers, CookieKind::PrimaryCeremony.cookie_name(cookies))
            .ok_or_else(|| {
                kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
            })?;
        let reference = services
            .authenticator
            .token_digest(
                TokenPurpose::Ceremony,
                SecretBytes::new(ceremony.as_bytes().to_vec()),
            )
            .await?;
        let browser = services
            .authenticator
            .token_digest(
                TokenPurpose::Browser,
                SecretBytes::new(browser.as_bytes().to_vec()),
            )
            .await?;
        let purpose = enrollment_purpose(&form.purpose)?;
        let proof = PendingCheck {
            reference,
            binding: AuthBinding {
                target: form.target,
                browser,
                session: form.session,
                purpose: AuthPurpose::Enrollment(purpose),
                stage: AuthStage::Passkey,
                epoch: SecurityEpoch::new(form.epoch),
                credential_revision: Some(form.credential_revision),
            },
            now: services.clock.now().await?,
        };
        (FinishPasskeyEnrollment {
            uow: services.uow.as_ref(),
            admins: services.admins.as_ref(),
            credentials: services.credentials.as_ref(),
            sessions: services.sessions.as_ref(),
            audit: services.audit.as_ref(),
            authenticator: services.authenticator.as_ref(),
            clock: services.clock.as_ref(),
            ids: services.ids.as_ref(),
            policy: &services.policy,
        })
        .execute(FinishPasskeyEnrollmentInput {
            proof,
            credential: form.credential,
            response: SecretBytes::new(form.response.into_bytes()),
        })
        .await
    }
    .await;
    match result {
        Ok(completed) => {
            let Ok(token) = cookie_value(&completed.token) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(cookie) = format_cookie_header(CookieKind::AdminSession, &token, cookies) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            (
                StatusCode::SEE_OTHER,
                [
                    (header::CACHE_CONTROL, "no-store"),
                    (header::LOCATION, "/console/admins"),
                    (header::SET_COOKIE, cookie.as_str()),
                ],
            )
                .into_response()
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            enrollment_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => enrollment_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct FinishTotpForm {
    credential: kernel::ids::CredentialId,
    target: AdminId,
    session: SessionId,
    epoch: u64,
    credential_revision: u64,
    purpose: String,
    code: String,
    csrf_token: String,
    lang: Option<String>,
}

/// Finish TOTP enrollment without exposing the credential envelope.
pub fn enroll_totp_finish_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/enrol/totp/finish", post(handle_totp_finish))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state((services, cookies))
}

async fn handle_totp_finish(
    State((services, cookies)): State<(ConsoleServices, CookieSecurityPolicy)>,
    headers: HeaderMap,
    Form(form): Form<FinishTotpForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return enrollment_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let ceremony =
            cookie(&headers, CookieKind::TotpCeremony.cookie_name(cookies)).ok_or_else(|| {
                kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
            })?;
        let browser = cookie(&headers, CookieKind::PrimaryCeremony.cookie_name(cookies))
            .ok_or_else(|| {
                kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
            })?;
        let reference = services
            .authenticator
            .token_digest(
                TokenPurpose::Ceremony,
                SecretBytes::new(ceremony.as_bytes().to_vec()),
            )
            .await?;
        let browser = services
            .authenticator
            .token_digest(
                TokenPurpose::Browser,
                SecretBytes::new(browser.as_bytes().to_vec()),
            )
            .await?;
        let proof = PendingCheck {
            reference,
            binding: AuthBinding {
                target: form.target,
                browser,
                session: form.session,
                purpose: AuthPurpose::Enrollment(enrollment_purpose(&form.purpose)?),
                stage: AuthStage::Totp,
                epoch: SecurityEpoch::new(form.epoch),
                credential_revision: Some(form.credential_revision),
            },
            now: services.clock.now().await?,
        };
        (FinishTotpEnrollment {
            uow: services.uow.as_ref(),
            admins: services.admins.as_ref(),
            credentials: services.credentials.as_ref(),
            sessions: services.sessions.as_ref(),
            audit: services.audit.as_ref(),
            authenticator: services.authenticator.as_ref(),
            clock: services.clock.as_ref(),
            ids: services.ids.as_ref(),
            policy: &services.policy,
        })
        .execute(FinishTotpEnrollmentInput {
            proof,
            credential: form.credential,
            code: SecretBytes::new(form.code.into_bytes()),
        })
        .await
    }
    .await;
    match result {
        Ok(completed) => {
            let Ok(token) = cookie_value(&completed.token) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(cookie) = format_cookie_header(CookieKind::AdminSession, &token, cookies) else {
                return enrollment_error(locale, StatusCode::UNAUTHORIZED);
            };
            (
                StatusCode::SEE_OTHER,
                [
                    (header::CACHE_CONTROL, "no-store"),
                    (header::LOCATION, "/console/admins"),
                    (header::SET_COOKIE, cookie.as_str()),
                ],
            )
                .into_response()
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            enrollment_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => enrollment_error(locale, StatusCode::UNAUTHORIZED),
    }
}

fn enrollment_purpose(value: &str) -> Result<EnrollmentPurpose, kernel::error::DomainError> {
    match value {
        "addition" => Ok(EnrollmentPurpose::Addition),
        "recovery" => Ok(EnrollmentPurpose::Recovery),
        _ => Err(kernel::error::DomainError::validation(
            ErrorCode::InvalidValue,
        )),
    }
}

async fn primary_pending_check(
    services: &ConsoleServices,
    headers: &HeaderMap,
    cookies: CookieSecurityPolicy,
    form: &ProofForm,
) -> Result<PendingCheck, kernel::error::DomainError> {
    let proof =
        cookie(headers, CookieKind::PrimaryProof.cookie_name(cookies)).ok_or_else(|| {
            kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
        })?;
    let browser =
        cookie(headers, CookieKind::PrimaryCeremony.cookie_name(cookies)).ok_or_else(|| {
            kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
        })?;
    let reference = services
        .authenticator
        .token_digest(
            TokenPurpose::PrimaryProof,
            SecretBytes::new(proof.as_bytes().to_vec()),
        )
        .await?;
    let browser = services
        .authenticator
        .token_digest(
            TokenPurpose::Browser,
            SecretBytes::new(browser.as_bytes().to_vec()),
        )
        .await?;
    let purpose = match form.purpose.as_str() {
        "addition" => EnrollmentPurpose::Addition,
        "recovery" => EnrollmentPurpose::Recovery,
        _ => {
            return Err(kernel::error::DomainError::validation(
                ErrorCode::InvalidValue,
            ));
        }
    };
    Ok(PendingCheck {
        reference,
        binding: AuthBinding {
            target: form.target,
            browser,
            session: form.session,
            purpose: AuthPurpose::Enrollment(purpose),
            stage: AuthStage::PrimaryVerified,
            epoch: SecurityEpoch::new(form.epoch),
            credential_revision: None,
        },
        now: services.clock.now().await?,
    })
}

fn enrollment_error(locale: Locale, status: StatusCode) -> Response {
    let message = ConsoleCatalog::load_authoritative()
        .ok()
        .and_then(|catalog| catalog.format(locale, "validation-invalid-value", &[]).ok())
        .unwrap_or_else(|| "Invalid request".to_owned());
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Html(format!("<!doctype html><html lang=\"{}\" dir=\"{}\"><main><p role=\"alert\">{}</p></main></html>", locale.as_tag(), locale.direction().as_str(), message)),
    )
        .into_response()
}
