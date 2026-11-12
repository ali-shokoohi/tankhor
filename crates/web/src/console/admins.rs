//! Current administrator management views; all authority is checked by application services.

use admin::{
    application::{
        invite_admin::{InviteAdmin, InviteAdminInput},
        list_admins::{ListAdmins, ListAdminsInput},
        recover_admin::{RecoverAdmin, RecoverAdminInput},
    },
    domain::{audit::ReasonCode, secrets::SecretBytes},
    ports::admin_authenticator::TokenPurpose,
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
use kernel::{error::ErrorCode, locale::Locale};
use serde::Deserialize;

use super::{ConsoleServices, auth::cookie};
use crate::{
    i18n::ConsoleCatalog,
    security::{
        CookieKind, CookieSecurityPolicy, CsrfContext, CsrfProtectionState, enforce_csrf_middleware,
    },
};

#[derive(Clone)]
struct AdminState {
    services: ConsoleServices,
    cookies: CookieSecurityPolicy,
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    lang: Option<String>,
    cursor: Option<String>,
    limit: Option<u16>,
}

#[derive(Template)]
#[template(path = "console/admins.html")]
struct AdminsView {
    lang: &'static str,
    dir: &'static str,
    heading: String,
    empty: String,
    invite_phone: String,
    invite_submit: String,
    csrf: Option<String>,
    admins: Vec<String>,
}

/// List administrators only after the `ListAdmins` service verifies current full access.
pub fn admin_list_router(services: ConsoleServices, cookies: CookieSecurityPolicy) -> Router {
    Router::new()
        .route("/console/admins", get(handle_admin_list))
        .with_state(AdminState { services, cookies })
}

async fn handle_admin_list(
    State(state): State<AdminState>,
    headers: HeaderMap,
    csrf: Option<Extension<CsrfContext>>,
    Query(query): Query<ListQuery>,
) -> Response {
    let locale = query
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    let result = async {
        let token = cookie(
            &headers,
            CookieKind::AdminSession.cookie_name(state.cookies),
        )
        .ok_or_else(|| {
            kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
        })?;
        let actor = state
            .services
            .authenticator
            .token_digest(
                TokenPurpose::Session,
                SecretBytes::new(token.as_bytes().to_vec()),
            )
            .await?;
        (ListAdmins {
            uow: state.services.uow.as_ref(),
            admins: state.services.admins.as_ref(),
            sessions: state.services.sessions.as_ref(),
            clock: state.services.clock.as_ref(),
        })
        .execute(ListAdminsInput {
            actor,
            cursor: query.cursor,
            limit: query.limit.unwrap_or(50),
        })
        .await
    }
    .await;
    match result {
        Ok(page) => {
            let rendered = ConsoleCatalog::load_authoritative()
                .ok()
                .and_then(|catalog| {
                    let heading = catalog.format(locale, "admins-heading", &[]).ok()?;
                    let empty = catalog.format(locale, "admins-empty", &[]).ok()?;
                    let invite_phone = catalog.format(locale, "invite-phone", &[]).ok()?;
                    let invite_submit = catalog.format(locale, "invite-submit", &[]).ok()?;
                    AdminsView {
                        lang: locale.as_tag(),
                        dir: locale.direction().as_str(),
                        heading,
                        empty,
                        invite_phone,
                        invite_submit,
                        csrf: csrf.and_then(|Extension(context)| {
                            String::from_utf8(context.expected_bytes().to_vec()).ok()
                        }),
                        admins: page
                            .entries
                            .iter()
                            .map(|entry| entry.id.to_string())
                            .collect(),
                    }
                    .render()
                    .ok()
                });
            match rendered {
                Some(html) => (
                    StatusCode::OK,
                    [(header::CACHE_CONTROL, "no-store")],
                    Html(html),
                )
                    .into_response(),
                None => localized_error(locale, StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            localized_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => localized_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct InviteForm {
    phone: String,
    csrf_token: String,
    lang: Option<String>,
}

/// Invite an administrator through the transactional `InviteAdmin` application service.
pub fn admin_invite_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/admins/invite", post(handle_admin_invite))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(AdminState { services, cookies })
}

async fn handle_admin_invite(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Form(form): Form<InviteForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return localized_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let token = cookie(
            &headers,
            CookieKind::AdminSession.cookie_name(state.cookies),
        )
        .ok_or_else(|| {
            kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
        })?;
        let actor = state
            .services
            .authenticator
            .token_digest(
                TokenPurpose::Session,
                SecretBytes::new(token.as_bytes().to_vec()),
            )
            .await?;
        (InviteAdmin {
            uow: state.services.uow.as_ref(),
            admins: state.services.admins.as_ref(),
            sessions: state.services.sessions.as_ref(),
            audit: state.services.audit.as_ref(),
            authenticator: state.services.authenticator.as_ref(),
            clock: state.services.clock.as_ref(),
            ids: state.services.ids.as_ref(),
            policy: &state.services.policy,
        })
        .execute(InviteAdminInput {
            actor,
            phone: SecretBytes::new(form.phone.into_bytes()),
            reason: Some(ReasonCode::AdminAddition),
        })
        .await
    }
    .await;
    match result {
        Ok(_) => (
            StatusCode::SEE_OTHER,
            [
                (header::CACHE_CONTROL, "no-store"),
                (header::LOCATION, "/console/admins"),
            ],
        )
            .into_response(),
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            localized_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(error) if error.code() == ErrorCode::IdempotencyConflict => {
            localized_error(locale, StatusCode::CONFLICT)
        }
        Err(_) => localized_error(locale, StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Deserialize)]
struct RecoverForm {
    target: kernel::ids::AdminId,
    csrf_token: String,
    lang: Option<String>,
}

/// Recover another administrator through the application-owned epoch transition.
pub fn admin_recover_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    Router::new()
        .route("/console/admins/recover", post(handle_admin_recover))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(AdminState { services, cookies })
}

async fn handle_admin_recover(
    State(state): State<AdminState>,
    headers: HeaderMap,
    Form(form): Form<RecoverForm>,
) -> Response {
    let locale = form
        .lang
        .as_deref()
        .and_then(|value| value.parse().ok())
        .unwrap_or_default();
    if form.csrf_token.is_empty() {
        return localized_error(locale, StatusCode::FORBIDDEN);
    }
    let result = async {
        let token = cookie(
            &headers,
            CookieKind::AdminSession.cookie_name(state.cookies),
        )
        .ok_or_else(|| {
            kernel::error::DomainError::unauthorised(ErrorCode::AuthenticationRequired)
        })?;
        let actor = state
            .services
            .authenticator
            .token_digest(
                TokenPurpose::Session,
                SecretBytes::new(token.as_bytes().to_vec()),
            )
            .await?;
        (RecoverAdmin {
            uow: state.services.uow.as_ref(),
            admins: state.services.admins.as_ref(),
            sessions: state.services.sessions.as_ref(),
            credentials: state.services.credentials.as_ref(),
            otp: state.services.otps.as_ref(),
            audit: state.services.audit.as_ref(),
            clock: state.services.clock.as_ref(),
            ids: state.services.ids.as_ref(),
            policy: &state.services.policy,
        })
        .execute(RecoverAdminInput {
            actor,
            target: form.target,
            reason: Some(ReasonCode::AdminRecovery),
        })
        .await
    }
    .await;
    match result {
        Ok(()) => (
            StatusCode::SEE_OTHER,
            [
                (header::CACHE_CONTROL, "no-store"),
                (header::LOCATION, "/console/admins"),
            ],
        )
            .into_response(),
        Err(error) if error.code() == ErrorCode::InvalidValue => {
            localized_error(locale, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Err(_) => localized_error(locale, StatusCode::UNAUTHORIZED),
    }
}

fn localized_error(locale: Locale, status: StatusCode) -> Response {
    let message = ConsoleCatalog::load_authoritative()
        .ok()
        .and_then(|catalog| catalog.format(locale, "validation-invalid-value", &[]).ok())
        .unwrap_or_else(|| "Invalid request".to_owned());
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Html(format!(
            "<!doctype html><html lang=\"{}\" dir=\"{}\"><main><p role=\"alert\">{}</p></main></html>",
            locale.as_tag(),
            locale.direction().as_str(),
            message
        )),
    )
        .into_response()
}
