//! Administrator access pages and forms.

use std::sync::Arc;

const PASSKEY_BRIDGE_JS: &str = include_str!("../../static/passkey.js");

use admin::application::passkey::{
    BeginPasskey, BeginPasskeyInput, FinishPasskey, FinishPasskeyInput,
};
use admin::application::prepare_access_form::PrepareAccessForm;
use admin::application::request_admin_otp::{RequestAdminOtp, RequestOtp};
use admin::application::sign_out::SignOut;
use admin::application::totp::{FinishTotp, FinishTotpInput};
use admin::application::verify_admin_otp::{VerifyAdminOtp, VerifyOtp};
use admin::domain::admin_state::SecurityEpoch;
use admin::domain::auth_state::{AuthBinding, AuthPurpose, AuthStage, BindingDigest};
use admin::domain::secrets::SecretBytes;
use admin::ports::admin_authenticator::{AdminAuthenticator, TokenPurpose};
use admin::ports::session_store::PendingCheck;
use askama::Template;
use axum::Router;
use axum::extract::{Extension, Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use kernel::error::{DomainError, ErrorCode};
use kernel::locale::Locale;
use serde::Deserialize;

use crate::console::ConsoleServices;
use crate::i18n::{ConsoleCatalog, I18nError};
use crate::security::{
    CookieKind, CookieSecurityPolicy, CsrfProtectionState, enforce_csrf_middleware,
    format_cookie_header,
};

#[derive(Clone)]
struct SignInState {
    authenticator: Arc<dyn AdminAuthenticator>,
    catalog: Arc<ConsoleCatalog>,
    cookies: CookieSecurityPolicy,
}

#[derive(Debug, Default, Deserialize)]
pub struct SignInQuery {
    lang: Option<String>,
    #[serde(rename = "return")]
    return_target: Option<String>,
}

#[derive(Template)]
#[template(path = "console/sign_in.html")]
struct SignInView {
    lang: &'static str,
    dir: &'static str,
    title: String,
    heading: String,
    introduction: String,
    phone_label: String,
    submit: String,
    csrf: String,
    return_target: String,
}

impl SignInView {
    fn new(
        catalog: &ConsoleCatalog,
        locale: Locale,
        csrf: String,
        return_target: String,
    ) -> Result<Self, I18nError> {
        Ok(Self {
            lang: locale.as_tag(),
            dir: locale.direction().as_str(),
            title: catalog.format(locale, "sign-in-title", &[])?,
            heading: catalog.format(locale, "sign-in-heading", &[])?,
            introduction: catalog.format(locale, "sign-in-introduction", &[])?,
            phone_label: catalog.format(locale, "sign-in-phone-label", &[])?,
            submit: catalog.format(locale, "sign-in-request-code", &[])?,
            csrf,
            return_target,
        })
    }
}

/// Build the public sign-in page. It issues form capabilities only, never authenticated access.
pub fn sign_in_router(
    authenticator: Arc<dyn AdminAuthenticator>,
    catalog: ConsoleCatalog,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/sign-in", get(handle_sign_in))
        .with_state(SignInState {
            authenticator,
            catalog: Arc::new(catalog),
            cookies,
        })
}

async fn handle_sign_in(
    State(state): State<SignInState>,
    Query(query): Query<SignInQuery>,
) -> Response {
    let locale = query
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    let return_target = safe_return_target(query.return_target.as_deref());
    let result = async {
        let browser = state
            .authenticator
            .issue_token(TokenPurpose::Browser)
            .await?;
        let form = (PrepareAccessForm {
            authenticator: state.authenticator.as_ref(),
        })
        .execute(browser.clone())
        .await?;
        let csrf = String::from_utf8(form.csrf.expose().to_vec()).map_err(|_| {
            kernel::error::DomainError::validation(kernel::error::ErrorCode::InvalidValue)
        })?;
        let view = SignInView::new(&state.catalog, locale, csrf, return_target).map_err(|_| {
            kernel::error::DomainError::validation(kernel::error::ErrorCode::InvalidValue)
        })?;
        let html = view.render().map_err(|_| {
            kernel::error::DomainError::validation(kernel::error::ErrorCode::InvalidValue)
        })?;
        let cookie = format_cookie_header(
            CookieKind::PrimaryCeremony,
            &cookie_value(&browser)?,
            state.cookies,
        )
        .map_err(|_| {
            kernel::error::DomainError::validation(kernel::error::ErrorCode::InvalidValue)
        })?;
        Ok::<_, kernel::error::DomainError>((html, cookie))
    }
    .await;
    match result {
        Ok((html, cookie)) => (
            StatusCode::OK,
            [
                (header::CACHE_CONTROL, "no-store"),
                (header::SET_COOKIE, cookie.as_str()),
            ],
            Html(html),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CACHE_CONTROL, "no-store")],
        )
            .into_response(),
    }
}

/// Serve the local, CSP-compatible passkey browser bridge without inline script.
pub fn passkey_bridge_router() -> Router {
    Router::new().route(
        "/console/static/passkey.js",
        get(|| async {
            (
                [(
                    header::CONTENT_TYPE,
                    "application/javascript; charset=utf-8",
                )],
                PASSKEY_BRIDGE_JS,
            )
        }),
    )
}

/// Keyed digest of the trusted network origin, installed by transport middleware.
#[derive(Debug, Clone)]
pub struct OriginBinding(pub BindingDigest);

#[derive(Clone)]
struct RequestCodeState {
    services: ConsoleServices,
    cookies: CookieSecurityPolicy,
}

#[derive(Debug, Deserialize)]
struct RequestCodeForm {
    csrf_token: String,
    phone: String,
    lang: Option<String>,
    #[serde(rename = "return")]
    return_target: Option<String>,
    predecessor: Option<String>,
}

/// Request a primary OTP without creating any session or protected access grant.
pub fn request_code_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/sign-in/request-code", post(handle_request_code))
        .layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(RequestCodeState { services, cookies })
}

async fn handle_request_code(
    State(state): State<RequestCodeState>,
    Extension(origin): Extension<OriginBinding>,
    headers: HeaderMap,
    Form(form): Form<RequestCodeForm>,
) -> Response {
    // The CSRF middleware validates this field before rebuilding the bounded body.
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return localized_form_error(locale, StatusCode::FORBIDDEN);
    }
    if form.phone.trim().is_empty() {
        return localized_form_error(locale, StatusCode::UNPROCESSABLE_ENTITY);
    }
    let result = async {
        let browser = browser_digest(&state.services, &headers, state.cookies).await?;
        let origin = origin.0;
        let predecessor = form
            .predecessor
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(|_| invalid())?;
        (RequestAdminOtp {
            uow: state.services.uow.as_ref(),
            admins: state.services.admins.as_ref(),
            rates: state.services.rates.as_ref(),
            otps: state.services.otps.as_ref(),
            authenticator: state.services.authenticator.as_ref(),
            clock: state.services.clock.as_ref(),
            ids: state.services.ids.as_ref(),
            primary: state.services.primary_sms.as_ref(),
            secondary: state.services.secondary_sms.as_ref(),
            policy: &state.services.policy,
        })
        .execute(RequestOtp {
            phone: SecretBytes::new(form.phone.into_bytes()),
            browser,
            origin,
            purpose: AuthPurpose::SignIn,
            predecessor,
        })
        .await
    }
    .await;
    match result {
        Ok(()) => {
            let target = safe_return_target(form.return_target.as_deref());
            let location = format!(
                "/console/sign-in/verify-code?lang={}&return={}",
                locale.as_tag(),
                encode_query(&target)
            );
            (
                StatusCode::SEE_OTHER,
                [
                    (header::CACHE_CONTROL, "no-store"),
                    (header::LOCATION, location.as_str()),
                ],
            )
                .into_response()
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            localized_form_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => localized_form_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct VerifyCodeForm {
    phone: String,
    challenge: String,
    code: String,
    csrf_token: String,
    lang: Option<String>,
    #[serde(rename = "return")]
    return_target: Option<String>,
}

/// Build the primary-code verification form action. Success creates only a restricted proof.
pub fn verify_code_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/sign-in/verify-code", post(handle_verify_code))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(RequestCodeState { services, cookies })
}

async fn handle_verify_code(
    State(state): State<RequestCodeState>,
    Extension(origin): Extension<OriginBinding>,
    headers: HeaderMap,
    Form(form): Form<VerifyCodeForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return localized_form_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let browser = browser_digest(&state.services, &headers, state.cookies).await?;
        let challenge = form.challenge.parse().map_err(|_| invalid())?;
        (VerifyAdminOtp {
            uow: state.services.uow.as_ref(),
            admins: state.services.admins.as_ref(),
            rates: state.services.rates.as_ref(),
            otps: state.services.otps.as_ref(),
            sessions: state.services.sessions.as_ref(),
            authenticator: state.services.authenticator.as_ref(),
            clock: state.services.clock.as_ref(),
            ids: state.services.ids.as_ref(),
            policy: &state.services.policy,
        })
        .execute(VerifyOtp {
            phone: SecretBytes::new(form.phone.into_bytes()),
            challenge,
            code: SecretBytes::new(form.code.into_bytes()),
            browser,
            origin: origin.0,
            purpose: AuthPurpose::SignIn,
        })
        .await
    }
    .await;
    match result {
        Ok(proof) => {
            let Ok(value) = cookie_value(&proof.token) else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(cookie) = format_cookie_header(CookieKind::PrimaryProof, &value, state.cookies)
            else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            let target = safe_return_target(form.return_target.as_deref());
            let location = format!(
                "/console/sign-in/second-factor?lang={}&return={}",
                locale.as_tag(),
                encode_query(&target)
            );
            (
                StatusCode::SEE_OTHER,
                [
                    (header::CACHE_CONTROL, "no-store".to_owned()),
                    (header::LOCATION, location),
                    (header::SET_COOKIE, cookie),
                ],
            )
                .into_response()
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            localized_form_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => localized_form_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct PasskeyBeginForm {
    credential: String,
    target: String,
    session: String,
    epoch: u64,
    csrf_token: String,
    lang: Option<String>,
}

#[derive(Template)]
#[template(path = "console/passkey.html")]
struct PasskeyView {
    lang: &'static str,
    dir: &'static str,
    heading: String,
    public_options: String,
    credential: String,
    target: String,
    session: String,
    epoch: u64,
    credential_revision: u64,
    csrf: String,
}

/// Begin a passkey ceremony from a current restricted primary proof.
pub fn passkey_begin_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/sign-in/passkey/begin", post(handle_passkey_begin))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(RequestCodeState { services, cookies })
}

async fn handle_passkey_begin(
    State(state): State<RequestCodeState>,
    headers: HeaderMap,
    Form(form): Form<PasskeyBeginForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return localized_form_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let reference = capability_digest(
            &state.services,
            &headers,
            state.cookies,
            CookieKind::PrimaryProof,
            TokenPurpose::PrimaryProof,
        )
        .await?;
        let browser = browser_digest(&state.services, &headers, state.cookies).await?;
        let credential = form.credential.parse().map_err(|_| invalid())?;
        let target = form.target.parse().map_err(|_| invalid())?;
        let session = form.session.parse().map_err(|_| invalid())?;
        let proof = PendingCheck {
            reference,
            binding: AuthBinding {
                target,
                browser,
                session,
                purpose: AuthPurpose::SignIn,
                stage: AuthStage::PrimaryVerified,
                epoch: SecurityEpoch::new(form.epoch),
                credential_revision: None,
            },
            now: time::OffsetDateTime::UNIX_EPOCH,
        };
        (BeginPasskey {
            uow: state.services.uow.as_ref(),
            admins: state.services.admins.as_ref(),
            credentials: state.services.credentials.as_ref(),
            sessions: state.services.sessions.as_ref(),
            authenticator: state.services.authenticator.as_ref(),
            clock: state.services.clock.as_ref(),
            ids: state.services.ids.as_ref(),
            policy: &state.services.policy,
        })
        .execute(BeginPasskeyInput { proof, credential })
        .await
    }
    .await;
    match result {
        Ok(options) => {
            let Ok(public_options) = cookie_value(&options.public_options) else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(token) = cookie_value(&options.token) else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(cookie) =
                format_cookie_header(CookieKind::PasskeyCeremony, &token, state.cookies)
            else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(catalog) = ConsoleCatalog::load_authoritative() else {
                return localized_form_error(locale, StatusCode::INTERNAL_SERVER_ERROR);
            };
            let view = PasskeyView {
                lang: locale.as_tag(),
                dir: locale.direction().as_str(),
                heading: catalog
                    .format(locale, "passkey-heading", &[])
                    .unwrap_or_else(|_| "Passkey verification".to_owned()),
                public_options,
                credential: form.credential,
                target: form.target,
                session: form.session,
                epoch: form.epoch,
                credential_revision: options.binding.credential_revision.unwrap_or_default(),
                csrf: form.csrf_token,
            };
            match view.render() {
                Ok(html) => (
                    StatusCode::OK,
                    [
                        (header::CACHE_CONTROL, "no-store".to_owned()),
                        (header::SET_COOKIE, cookie),
                    ],
                    Html(html),
                )
                    .into_response(),
                Err(_) => localized_form_error(locale, StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            localized_form_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => localized_form_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct PasskeyFinishForm {
    credential: String,
    target: String,
    session: String,
    epoch: u64,
    credential_revision: u64,
    response: String,
    csrf_token: String,
    lang: Option<String>,
    #[serde(rename = "return")]
    return_target: Option<String>,
}

/// Finish the current passkey ceremony and establish an audited full session.
pub fn passkey_finish_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route(
            "/console/sign-in/passkey/finish",
            post(handle_passkey_finish),
        )
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(RequestCodeState { services, cookies })
}

async fn handle_passkey_finish(
    State(state): State<RequestCodeState>,
    headers: HeaderMap,
    Form(form): Form<PasskeyFinishForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return localized_form_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let reference = capability_digest(
            &state.services,
            &headers,
            state.cookies,
            CookieKind::PasskeyCeremony,
            TokenPurpose::Ceremony,
        )
        .await?;
        let browser = browser_digest(&state.services, &headers, state.cookies).await?;
        let proof = PendingCheck {
            reference,
            binding: AuthBinding {
                target: form.target.parse().map_err(|_| invalid())?,
                browser,
                session: form.session.parse().map_err(|_| invalid())?,
                purpose: AuthPurpose::SignIn,
                stage: AuthStage::Passkey,
                epoch: SecurityEpoch::new(form.epoch),
                credential_revision: Some(form.credential_revision),
            },
            now: time::OffsetDateTime::UNIX_EPOCH,
        };
        (FinishPasskey {
            uow: state.services.uow.as_ref(),
            admins: state.services.admins.as_ref(),
            credentials: state.services.credentials.as_ref(),
            sessions: state.services.sessions.as_ref(),
            audit: state.services.audit.as_ref(),
            authenticator: state.services.authenticator.as_ref(),
            clock: state.services.clock.as_ref(),
            ids: state.services.ids.as_ref(),
            policy: &state.services.policy,
        })
        .execute(FinishPasskeyInput {
            proof,
            credential: form.credential.parse().map_err(|_| invalid())?,
            response: SecretBytes::new(form.response.into_bytes()),
        })
        .await
    }
    .await;
    match result {
        Ok(session) => {
            let Ok(token) = cookie_value(&session.token) else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(cookie) = format_cookie_header(CookieKind::AdminSession, &token, state.cookies)
            else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            (
                StatusCode::SEE_OTHER,
                [
                    (header::CACHE_CONTROL, "no-store".to_owned()),
                    (
                        header::LOCATION,
                        safe_return_target(form.return_target.as_deref()),
                    ),
                    (header::SET_COOKIE, cookie),
                ],
            )
                .into_response()
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            localized_form_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => localized_form_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct SecondFactorQuery {
    target: kernel::ids::AdminId,
    session: kernel::ids::SessionId,
    epoch: u64,
    lang: Option<String>,
}

#[derive(Template)]
#[template(path = "console/second_factor.html")]
struct SecondFactorView {
    lang: &'static str,
    dir: &'static str,
    heading: String,
    code_label: String,
    submit: String,
    guidance: String,
    target: kernel::ids::AdminId,
    session: kernel::ids::SessionId,
    epoch: u64,
    csrf: String,
}

/// Render the complete, no-JavaScript second-factor form without granting access.
pub fn second_factor_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/sign-in/second-factor", get(handle_second_factor))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(RequestCodeState { services, cookies })
}

async fn handle_second_factor(
    State(state): State<RequestCodeState>,
    headers: HeaderMap,
    csrf: Option<Extension<crate::security::CsrfContext>>,
    Query(query): Query<SecondFactorQuery>,
) -> Response {
    let locale = query
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    let result = async {
        capability_digest(
            &state.services,
            &headers,
            state.cookies,
            CookieKind::PrimaryProof,
            TokenPurpose::PrimaryProof,
        )
        .await?;
        browser_digest(&state.services, &headers, state.cookies).await
    }
    .await;
    if result.is_err() {
        return localized_form_error(locale, StatusCode::UNAUTHORIZED);
    }
    let Ok(catalog) = ConsoleCatalog::load_authoritative() else {
        return localized_form_error(locale, StatusCode::INTERNAL_SERVER_ERROR);
    };
    let text = |key| {
        catalog
            .format(locale, key, &[])
            .unwrap_or_else(|_| key.to_owned())
    };
    let csrf = csrf.and_then(|Extension(context)| {
        std::str::from_utf8(context.expected_bytes())
            .ok()
            .map(str::to_owned)
    });
    let Some(csrf) = csrf else {
        return localized_form_error(locale, StatusCode::FORBIDDEN);
    };
    let view = SecondFactorView {
        lang: locale.as_tag(),
        dir: locale.direction().as_str(),
        heading: text("second-factor-heading"),
        code_label: text("verify-code-label"),
        submit: text("second-factor-submit"),
        guidance: text("passkey-disabled-guidance"),
        target: query.target,
        session: query.session,
        epoch: query.epoch,
        csrf,
    };
    match view.render() {
        Ok(html) => (
            StatusCode::OK,
            [(header::CACHE_CONTROL, "no-store")],
            Html(html),
        )
            .into_response(),
        Err(_) => localized_form_error(locale, StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Debug, Deserialize)]
struct TotpForm {
    credential: String,
    target: String,
    session: String,
    epoch: u64,
    credential_revision: u64,
    code: String,
    csrf_token: String,
    lang: Option<String>,
    #[serde(rename = "return")]
    return_target: Option<String>,
}

/// Complete a primary-proof-bound TOTP sign-in and establish an audited session.
pub fn totp_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/sign-in/totp", post(handle_totp))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(RequestCodeState { services, cookies })
}

async fn handle_totp(
    State(state): State<RequestCodeState>,
    headers: HeaderMap,
    Form(form): Form<TotpForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return localized_form_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let reference = capability_digest(
            &state.services,
            &headers,
            state.cookies,
            CookieKind::PrimaryProof,
            TokenPurpose::PrimaryProof,
        )
        .await?;
        let browser = browser_digest(&state.services, &headers, state.cookies).await?;
        let proof = PendingCheck {
            reference,
            binding: AuthBinding {
                target: form.target.parse().map_err(|_| invalid())?,
                browser,
                session: form.session.parse().map_err(|_| invalid())?,
                purpose: AuthPurpose::SignIn,
                stage: AuthStage::PrimaryVerified,
                epoch: SecurityEpoch::new(form.epoch),
                credential_revision: Some(form.credential_revision),
            },
            now: time::OffsetDateTime::UNIX_EPOCH,
        };
        (FinishTotp {
            uow: state.services.uow.as_ref(),
            admins: state.services.admins.as_ref(),
            credentials: state.services.credentials.as_ref(),
            sessions: state.services.sessions.as_ref(),
            audit: state.services.audit.as_ref(),
            authenticator: state.services.authenticator.as_ref(),
            clock: state.services.clock.as_ref(),
            ids: state.services.ids.as_ref(),
            policy: &state.services.policy,
        })
        .execute(FinishTotpInput {
            proof,
            credential: form.credential.parse().map_err(|_| invalid())?,
            code: SecretBytes::new(form.code.into_bytes()),
        })
        .await
    }
    .await;
    match result {
        Ok(session) => {
            let Ok(token) = cookie_value(&session.token) else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            let Ok(cookie) = format_cookie_header(CookieKind::AdminSession, &token, state.cookies)
            else {
                return localized_form_error(locale, StatusCode::UNAUTHORIZED);
            };
            (
                StatusCode::SEE_OTHER,
                [
                    (header::CACHE_CONTROL, "no-store".to_owned()),
                    (
                        header::LOCATION,
                        safe_return_target(form.return_target.as_deref()),
                    ),
                    (header::SET_COOKIE, cookie),
                ],
            )
                .into_response()
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            localized_form_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => localized_form_error(locale, StatusCode::UNAUTHORIZED),
    }
}

async fn capability_digest(
    services: &ConsoleServices,
    headers: &HeaderMap,
    cookies: CookieSecurityPolicy,
    kind: CookieKind,
    purpose: TokenPurpose,
) -> Result<BindingDigest, DomainError> {
    let token = cookie(headers, kind.cookie_name(cookies))
        .ok_or_else(|| DomainError::unauthorised(ErrorCode::AuthenticationRequired))?;
    services
        .authenticator
        .token_digest(purpose, SecretBytes::new(token.as_bytes().to_vec()))
        .await
}

fn invalid() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

async fn browser_digest(
    services: &ConsoleServices,
    headers: &HeaderMap,
    cookies: CookieSecurityPolicy,
) -> Result<BindingDigest, DomainError> {
    let name = CookieKind::PrimaryCeremony.cookie_name(cookies);
    let token = cookie(headers, name)
        .ok_or_else(|| DomainError::unauthorised(ErrorCode::AuthenticationRequired))?;
    services
        .authenticator
        .token_digest(
            TokenPurpose::Browser,
            SecretBytes::new(token.as_bytes().to_vec()),
        )
        .await
}

pub(crate) fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then_some(value)
        })
}

fn encode_query(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[derive(Debug, Deserialize)]
struct SignOutForm {
    csrf_token: String,
    lang: Option<String>,
}

/// Revoke only the presented, currently authorized administrator session.
pub fn sign_out_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/sign-out", post(handle_sign_out))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state((services, cookies))
}

async fn handle_sign_out(
    State((services, cookies)): State<(ConsoleServices, CookieSecurityPolicy)>,
    headers: HeaderMap,
    Form(form): Form<SignOutForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return localized_form_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let token = cookie(&headers, CookieKind::AdminSession.cookie_name(cookies))
            .ok_or_else(|| DomainError::unauthorised(ErrorCode::AuthenticationRequired))?;
        (SignOut {
            uow: services.uow.as_ref(),
            admins: services.admins.as_ref(),
            sessions: services.sessions.as_ref(),
            authenticator: services.authenticator.as_ref(),
            clock: services.clock.as_ref(),
        })
        .execute(SecretBytes::new(token.as_bytes().to_vec()))
        .await
    }
    .await;
    match result {
        Ok(()) => {
            let clear = format!(
                "{}=; Max-Age=0; Path=/; HttpOnly; Secure; SameSite=Lax",
                CookieKind::AdminSession.cookie_name(cookies)
            );
            (
                StatusCode::SEE_OTHER,
                [
                    (header::CACHE_CONTROL, "no-store"),
                    (header::LOCATION, "/console/sign-in"),
                    (header::SET_COOKIE, clear.as_str()),
                ],
            )
                .into_response()
        }
        Err(_) => localized_form_error(locale, StatusCode::UNAUTHORIZED),
    }
}

fn localized_form_error(locale: Locale, status: StatusCode) -> Response {
    let catalog = ConsoleCatalog::load_authoritative();
    let message = catalog
        .ok()
        .and_then(|catalog| catalog.format(locale, "validation-invalid-value", &[]).ok())
        .unwrap_or_else(|| "Invalid request".to_owned());
    (status, [(header::CACHE_CONTROL, "no-store")], Html(format!("<!doctype html><html lang=\"{}\" dir=\"{}\"><main><p role=\"alert\">{}</p></main></html>", locale.as_tag(), locale.direction().as_str(), message))).into_response()
}

pub(super) fn cookie_value(value: &SecretBytes) -> Result<String, kernel::error::DomainError> {
    String::from_utf8(value.expose().to_vec())
        .map_err(|_| kernel::error::DomainError::validation(kernel::error::ErrorCode::InvalidValue))
}

/// Restrict post-authentication navigation to an absolute local console path.
#[must_use]
pub fn safe_return_target(value: Option<&str>) -> String {
    const DEFAULT: &str = "/console/admins";
    let Some(value) = value else {
        return DEFAULT.to_owned();
    };
    if value.starts_with("/console")
        && !value.starts_with("//")
        && !value.contains(['\\', '\r', '\n', '\0'])
        && value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        value.to_owned()
    } else {
        DEFAULT.to_owned()
    }
}
