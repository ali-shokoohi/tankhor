//! Console change draft review and impact preview handlers (`GET /console/changes/{id}` and `POST /console/changes/{id}/preview`).
//! FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41,
//! AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046.
use std::collections::BTreeMap;
use std::sync::Arc;

use admin::{
    application::{
        console_services::ConsoleServices,
        get_change::{ChangeDetails, GetChange, GetChangeInput},
        get_setting::{GetSetting, GetSettingInput},
        preview::{Preview, PreviewEvidence, PreviewInput},
    },
    domain::{
        auth_state::BindingDigest,
        change_preview::PreviewReport,
        change_revision::{ChangeSubject, ChangeValue},
        formats::{FormatSet, ImageFormat},
        impact_attempts::{GenerationUnitSnapshot, UnitQueueState},
        impact_cards::CardProjectSnapshot,
        impact_lifetime::LifetimePopulation,
        impact_photos::PhotoAccountSnapshot,
        impact_recovery::PendingRecoverySnapshot,
        impact_uploads::UploadPolicySnapshot,
        secrets::SecretBytes,
        settings::{SettingKey, SettingValue},
    },
    ports::admin_authenticator::TokenPurpose,
};
use askama::Template;
use axum::{
    Router,
    body::Bytes,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use kernel::{
    error::{DomainError, ErrorCategory, ErrorCode},
    locale::Locale,
};
use time::OffsetDateTime;

use crate::{
    i18n::ConsoleCatalog,
    security::{
        CookieKind, CookieSecurityPolicy, CsrfContext, CsrfProtectionState, enforce_csrf_middleware,
    },
};

#[derive(Clone)]
struct ChangesState {
    services: ConsoleServices,
    catalog: Arc<ConsoleCatalog>,
    cookies: CookieSecurityPolicy,
}

#[derive(Debug, Default, serde::Deserialize)]
struct ChangeQuery {
    lang: Option<String>,
    theme: Option<String>,
    viewport: Option<String>,
}

#[derive(Template)]
#[template(path = "console/change.html")]
struct ChangePageTemplate<'a> {
    lang: &'a str,
    dir: &'a str,
    theme: &'a str,
    viewport: &'a str,
    title: String,
    heading: String,
    subject_label: String,
    subject_key: String,
    change_id_label: String,
    change_id: String,
    revision_label: String,
    revision_id: String,
    revision_number: u32,
    baseline_label: String,
    baseline_version: String,
    live_value_label: String,
    live_value: String,
    proposed_value_label: String,
    proposed_value: String,
    detail_action: String,
    preview_action: String,
    csrf_token: String,
    submitted_revision: String,
    viewport_label: String,
    viewport_desktop_label: String,
    viewport_mobile_label: String,
    preview_submit_label: String,
    impact_heading: String,
    impact_none: String,
    has_preview: bool,
    preview_id: String,
    effect_rule: String,
    impact_explanation: String,
    is_lifetime: bool,
    changed_count_label: String,
    changed_count_display: String,
    already_due_count_label: String,
    already_due_count_display: String,
    conflict_error: String,
}

/// Builds the `GET /console/changes/{id}` router.
///
/// # Panics
/// Panics if the embedded authoritative Fluent catalog fails to load.
pub fn change_detail_router(services: ConsoleServices, cookies: CookieSecurityPolicy) -> Router {
    let catalog = Arc::new(ConsoleCatalog::load_authoritative().expect("console catalog"));
    Router::new()
        .route("/console/changes/{id}", get(handle_get_change))
        .with_state(ChangesState {
            services,
            catalog,
            cookies,
        })
}

/// Builds the CSRF-protected `POST /console/changes/{id}/preview` router.
///
/// # Panics
/// Panics if the embedded authoritative Fluent catalog fails to load.
pub fn change_preview_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    let catalog = Arc::new(ConsoleCatalog::load_authoritative().expect("console catalog"));
    Router::new()
        .route("/console/changes/{id}/preview", post(handle_preview))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(ChangesState {
            services,
            catalog,
            cookies,
        })
}

/// Builds the combined change review and preview router.
pub fn changes_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    change_detail_router(services.clone(), cookies)
        .merge(change_preview_router(services, csrf, cookies))
}

async fn handle_get_change(
    State(state): State<ChangesState>,
    Path(id): Path<String>,
    Query(query): Query<ChangeQuery>,
    headers: HeaderMap,
    csrf: Option<Extension<CsrfContext>>,
) -> Response {
    let locale = resolve_locale(query.lang.as_deref(), &headers);
    let theme = resolve_theme(query.theme.as_deref());
    let viewport = resolve_viewport(query.viewport.as_deref());
    let Some(actor) = extract_session_digest(&state.services, state.cookies, &headers).await else {
        return no_store((StatusCode::UNAUTHORIZED, Html(String::new())).into_response());
    };
    let get_change = GetChange {
        uow: state.services.uow.as_ref(),
        admins: state.services.admins.as_ref(),
        sessions: state.services.sessions.as_ref(),
        changes: state.services.changes.as_ref(),
        clock: state.services.clock.as_ref(),
    };
    match get_change
        .execute(GetChangeInput {
            actor: actor.clone(),
            change: id,
        })
        .await
    {
        Ok(details) => {
            let live_value = resolve_live_value(&state.services, actor, details.subject).await;
            let csrf_token = csrf
                .and_then(|Extension(ctx)| String::from_utf8(ctx.expected_bytes().to_vec()).ok())
                .unwrap_or_default();
            render_change_page(
                StatusCode::OK,
                &state.catalog,
                locale,
                theme,
                viewport,
                &details,
                &live_value,
                &csrf_token,
                None,
                String::new(),
            )
        }
        Err(error) => map_change_error(error),
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_preview(
    State(state): State<ChangesState>,
    Path(id): Path<String>,
    Query(query): Query<ChangeQuery>,
    headers: HeaderMap,
    csrf: Option<Extension<CsrfContext>>,
    body: Bytes,
) -> Response {
    let Some(actor) = extract_session_digest(&state.services, state.cookies, &headers).await else {
        return no_store((StatusCode::UNAUTHORIZED, Html(String::new())).into_response());
    };
    let fields = parse_form_fields(&body);
    let form_lang = first_field(&fields, "lang").or(query.lang.as_deref());
    let locale = resolve_locale(form_lang, &headers);
    let theme = resolve_theme(query.theme.as_deref());
    let viewport = resolve_viewport(first_field(&fields, "viewport").or(query.viewport.as_deref()));
    let submitted_revision = first_field(&fields, "revision")
        .unwrap_or_default()
        .trim()
        .to_owned();
    let csrf_token = csrf
        .and_then(|Extension(ctx)| String::from_utf8(ctx.expected_bytes().to_vec()).ok())
        .or_else(|| first_field(&fields, "csrf_token").map(ToOwned::to_owned))
        .unwrap_or_default();

    let get_change = GetChange {
        uow: state.services.uow.as_ref(),
        admins: state.services.admins.as_ref(),
        sessions: state.services.sessions.as_ref(),
        changes: state.services.changes.as_ref(),
        clock: state.services.clock.as_ref(),
    };
    let details = match get_change
        .execute(GetChangeInput {
            actor: actor.clone(),
            change: id.clone(),
        })
        .await
    {
        Ok(details) => details,
        Err(error) => return map_change_error(error),
    };

    let now = state
        .services
        .clock
        .now()
        .await
        .unwrap_or(OffsetDateTime::UNIX_EPOCH);
    let evidence = default_preview_evidence(details.subject, now);
    let preview = Preview {
        uow: state.services.uow.as_ref(),
        sessions: state.services.sessions.as_ref(),
        admins: state.services.admins.as_ref(),
        settings: state.services.settings.as_ref(),
        flags: state.services.flags.as_ref(),
        changes: state.services.changes.as_ref(),
        audit: state.services.audit.as_ref(),
        clock: state.services.clock.as_ref(),
        ids: state.services.ids.as_ref(),
    };

    match preview
        .execute(PreviewInput {
            actor: actor.clone(),
            change: id.clone(),
            revision: submitted_revision.clone(),
            evidence,
        })
        .await
    {
        Ok(_) => {
            let refreshed = match get_change
                .execute(GetChangeInput {
                    actor: actor.clone(),
                    change: id,
                })
                .await
            {
                Ok(details) => details,
                Err(error) => return map_change_error(error),
            };
            let live_value = resolve_live_value(&state.services, actor, refreshed.subject).await;
            render_change_page(
                StatusCode::OK,
                &state.catalog,
                locale,
                theme,
                viewport,
                &refreshed,
                &live_value,
                &csrf_token,
                None,
                String::new(),
            )
        }
        Err(error)
            if error.category() == ErrorCategory::Conflict
                || error.code() == ErrorCode::StaleRevision =>
        {
            let conflict_msg = state
                .catalog
                .format(locale, "change-preview-conflict", &[])
                .unwrap_or_default();
            let live_value = resolve_live_value(&state.services, actor, details.subject).await;
            render_change_page(
                StatusCode::CONFLICT,
                &state.catalog,
                locale,
                theme,
                viewport,
                &details,
                &live_value,
                &csrf_token,
                Some(submitted_revision),
                conflict_msg,
            )
        }
        Err(error) => map_change_error(error),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn render_change_page(
    status: StatusCode,
    catalog: &ConsoleCatalog,
    locale: Locale,
    theme: &str,
    viewport: &str,
    details: &ChangeDetails,
    live_value: &str,
    csrf_token: &str,
    preserved_revision: Option<String>,
    conflict_error: String,
) -> Response {
    let (subject_key, subject_label_key) = match details.subject {
        ChangeSubject::Setting(key) => (
            key.as_str().to_owned(),
            format!("setting-{}-label", key.as_str()),
        ),
        ChangeSubject::Flag(key) => (key.as_str().to_owned(), key.as_str().to_owned()),
    };
    let subject_label = catalog
        .format(locale, &subject_label_key, &[])
        .unwrap_or_else(|_| subject_key.clone());
    let (revision_id, revision_number, proposed_value) = details.current.as_ref().map_or_else(
        || (String::new(), 0, String::new()),
        |rev| {
            (
                rev.id.to_string(),
                rev.number,
                format_change_value(rev.value),
            )
        },
    );
    let submitted_revision = preserved_revision.unwrap_or_else(|| revision_id.clone());

    let (
        has_preview,
        preview_id,
        effect_rule,
        impact_explanation,
        is_lifetime,
        changed_count_display,
        already_due_count_display,
    ) = details.preview.as_ref().map_or_else(
        || {
            (
                false,
                String::new(),
                String::new(),
                String::new(),
                false,
                String::new(),
                String::new(),
            )
        },
        |report| format_preview_summary(catalog, locale, details.subject, report),
    );

    let template = ChangePageTemplate {
        lang: locale.as_tag(),
        dir: locale.direction().as_str(),
        theme,
        viewport,
        title: catalog
            .format(locale, "change-page-title", &[])
            .unwrap_or_default(),
        heading: catalog
            .format(locale, "change-page-heading", &[])
            .unwrap_or_default(),
        subject_label,
        subject_key,
        change_id_label: catalog
            .format(locale, "change-id-label", &[])
            .unwrap_or_default(),
        change_id: details.id.to_string(),
        revision_label: catalog
            .format(locale, "change-revision-label", &[])
            .unwrap_or_default(),
        revision_id,
        revision_number,
        baseline_label: catalog
            .format(locale, "change-baseline-label", &[])
            .unwrap_or_default(),
        baseline_version: details.baseline_version.to_string(),
        live_value_label: catalog
            .format(locale, "change-live-value-label", &[])
            .unwrap_or_default(),
        live_value: live_value.to_owned(),
        proposed_value_label: catalog
            .format(locale, "change-proposed-value-label", &[])
            .unwrap_or_default(),
        proposed_value,
        detail_action: format!("/console/changes/{}", details.id),
        preview_action: format!("/console/changes/{}/preview", details.id),
        csrf_token: csrf_token.to_owned(),
        submitted_revision,
        viewport_label: catalog
            .format(locale, "change-viewport-label", &[])
            .unwrap_or_default(),
        viewport_desktop_label: catalog
            .format(locale, "change-viewport-desktop", &[])
            .unwrap_or_default(),
        viewport_mobile_label: catalog
            .format(locale, "change-viewport-mobile", &[])
            .unwrap_or_default(),
        preview_submit_label: catalog
            .format(locale, "change-preview-submit", &[])
            .unwrap_or_default(),
        impact_heading: catalog
            .format(locale, "change-impact-heading", &[])
            .unwrap_or_default(),
        impact_none: catalog
            .format(locale, "change-impact-none", &[])
            .unwrap_or_default(),
        has_preview,
        preview_id,
        effect_rule,
        impact_explanation,
        is_lifetime,
        changed_count_label: catalog
            .format(locale, "change-count-changed-label", &[])
            .unwrap_or_default(),
        changed_count_display,
        already_due_count_label: catalog
            .format(locale, "change-count-already-due-label", &[])
            .unwrap_or_default(),
        already_due_count_display,
        conflict_error,
    };
    render_html(status, &template)
}

fn format_preview_summary(
    catalog: &ConsoleCatalog,
    locale: Locale,
    subject: ChangeSubject,
    report: &PreviewReport,
) -> (bool, String, String, String, bool, String, String) {
    let rule = report.effect_rule().as_str().to_owned();
    let explanation_key = match subject {
        ChangeSubject::Setting(
            SettingKey::BuyerTempPhotoDays | SettingKey::StudioTempUploadDays,
        ) => {
            if rule.starts_with("no-existing-expiry-reduction") {
                "change-lifetime-non-destructive"
            } else {
                "change-lifetime-original-upload"
            }
        }
        ChangeSubject::Setting(SettingKey::StudioAutoDraftDays) => {
            if rule.starts_with("no-existing-expiry-reduction") {
                "change-lifetime-non-destructive"
            } else {
                "change-lifetime-last-activity"
            }
        }
        ChangeSubject::Setting(SettingKey::StudioMaxCards) => "change-preserve-cards",
        ChangeSubject::Setting(SettingKey::BuyerMaxPermanentPhotos) => "change-preserve-photos",
        ChangeSubject::Setting(
            SettingKey::UploadMaxMb
            | SettingKey::UploadMinShortestEdgePx
            | SettingKey::UploadFormats,
        ) => "change-preserve-uploads",
        ChangeSubject::Setting(SettingKey::GenerationMaxAttempts) => "change-preserve-attempts",
        ChangeSubject::Setting(SettingKey::AccountRecoveryDays) => "change-preserve-recovery",
        ChangeSubject::Flag(_) => "change-flag-not-delivered",
    };
    let explanation = catalog
        .format(locale, explanation_key, &[])
        .unwrap_or_default();

    let is_lifetime = matches!(
        subject,
        ChangeSubject::Setting(
            SettingKey::BuyerTempPhotoDays
                | SettingKey::StudioTempUploadDays
                | SettingKey::StudioAutoDraftDays
        )
    );
    let unknown_text = catalog
        .format(locale, "change-count-unknown", &[])
        .unwrap_or_default();
    let changed_display = if is_lifetime {
        report
            .evidence()
            .first()
            .and_then(admin::domain::change_preview::PopulationEvidence::known_count)
            .map_or_else(|| unknown_text.clone(), |count| count.to_string())
    } else {
        String::new()
    };
    let already_due_display = if is_lifetime {
        report
            .evidence()
            .get(1)
            .and_then(admin::domain::change_preview::PopulationEvidence::known_count)
            .map_or_else(|| unknown_text, |count| count.to_string())
    } else {
        String::new()
    };

    (
        true,
        report.id().to_string(),
        rule,
        explanation,
        is_lifetime,
        changed_display,
        already_due_display,
    )
}

fn default_preview_evidence(subject: ChangeSubject, now: OffsetDateTime) -> PreviewEvidence {
    match subject {
        ChangeSubject::Setting(
            SettingKey::BuyerTempPhotoDays
            | SettingKey::StudioTempUploadDays
            | SettingKey::StudioAutoDraftDays,
        ) => PreviewEvidence::Lifetime(LifetimePopulation::Unavailable),
        ChangeSubject::Setting(SettingKey::StudioMaxCards) => {
            PreviewEvidence::Card(CardProjectSnapshot::new(0, false))
        }
        ChangeSubject::Setting(SettingKey::BuyerMaxPermanentPhotos) => {
            PreviewEvidence::Photo(PhotoAccountSnapshot::new(0))
        }
        ChangeSubject::Setting(
            SettingKey::UploadMaxMb
            | SettingKey::UploadMinShortestEdgePx
            | SettingKey::UploadFormats,
        ) => PreviewEvidence::Upload(UploadPolicySnapshot::new(15, 512, FormatSet::all())),
        ChangeSubject::Setting(SettingKey::GenerationMaxAttempts) => PreviewEvidence::Attempt(
            GenerationUnitSnapshot::new(UnitQueueState::PreviouslyQueued, 3),
        ),
        ChangeSubject::Setting(SettingKey::AccountRecoveryDays) => {
            PreviewEvidence::Recovery(PendingRecoverySnapshot::new(now))
        }
        ChangeSubject::Flag(_) => PreviewEvidence::None,
    }
}

async fn resolve_live_value(
    services: &ConsoleServices,
    actor: BindingDigest,
    subject: ChangeSubject,
) -> String {
    match subject {
        ChangeSubject::Setting(key) => {
            let get_setting = GetSetting {
                uow: services.uow.as_ref(),
                admins: services.admins.as_ref(),
                sessions: services.sessions.as_ref(),
                settings: services.settings.as_ref(),
                clock: services.clock.as_ref(),
            };
            get_setting
                .execute(GetSettingInput {
                    actor,
                    key: key.as_str().to_owned(),
                })
                .await
                .map_or_else(
                    |_| String::new(),
                    |summary| format_setting_value(&summary.value),
                )
        }
        ChangeSubject::Flag(_) => "false".to_owned(),
    }
}

fn format_setting_value(value: &SettingValue) -> String {
    match value {
        SettingValue::Number(n) => n.to_string(),
        SettingValue::Boolean(b) => b.to_string(),
        SettingValue::Formats(formats) => formats
            .formats()
            .into_iter()
            .map(ImageFormat::as_str)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn format_change_value(value: ChangeValue) -> String {
    match value {
        ChangeValue::Number(n) => n.to_string(),
        ChangeValue::Formats(formats) => format_setting_value(&SettingValue::Formats(formats)),
        ChangeValue::Boolean(flag) => flag.to_string(),
    }
}

fn resolve_locale(lang_param: Option<&str>, headers: &HeaderMap) -> Locale {
    if let Some(tag) = lang_param
        && let Ok(locale) = tag.parse::<Locale>()
    {
        return locale;
    }
    if let Some(accept) = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
    {
        if accept.to_ascii_lowercase().starts_with("en") {
            return Locale::En;
        }
        if accept.to_ascii_lowercase().starts_with("fa") {
            return Locale::Fa;
        }
    }
    Locale::Fa
}

fn resolve_theme(theme: Option<&str>) -> &'static str {
    match theme {
        Some("dark") => "dark",
        Some("system") => "system",
        _ => "light",
    }
}

fn resolve_viewport(viewport: Option<&str>) -> &'static str {
    match viewport {
        Some("mobile") => "mobile",
        _ => "desktop",
    }
}

async fn extract_session_digest(
    services: &ConsoleServices,
    cookies: CookieSecurityPolicy,
    headers: &HeaderMap,
) -> Option<BindingDigest> {
    let raw = extract_cookie(headers, CookieKind::AdminSession.cookie_name(cookies))?;
    services
        .authenticator
        .token_digest(TokenPurpose::Session, SecretBytes::new(raw.into_bytes()))
        .await
        .ok()
}

fn extract_cookie(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';').find_map(|pair| {
        let (k, v) = pair.trim().split_once('=')?;
        (k.trim() == cookie_name && !v.trim().is_empty()).then(|| v.trim().to_owned())
    })
}

fn parse_form_fields(body: &Bytes) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let Ok(raw) = std::str::from_utf8(body) else {
        return map;
    };
    for pair in raw.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let key = percent_decode(k);
        let value = percent_decode(v);
        map.entry(key).or_default().push(value);
    }
    map
}

fn first_field<'a>(fields: &'a BTreeMap<String, Vec<String>>, key: &str) -> Option<&'a str> {
    fields.get(key).and_then(|v| v.first()).map(String::as_str)
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut idx = 0;
    while idx < bytes.len() {
        match bytes[idx] {
            b'+' => {
                out.push(b' ');
                idx += 1;
            }
            b'%' if idx + 2 < bytes.len() => {
                if let (Some(hi), Some(lo)) = (from_hex(bytes[idx + 1]), from_hex(bytes[idx + 2])) {
                    out.push((hi << 4) | lo);
                    idx += 3;
                } else {
                    out.push(bytes[idx]);
                    idx += 1;
                }
            }
            byte => {
                out.push(byte);
                idx += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

const fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn map_change_error(error: DomainError) -> Response {
    let status = match (error.category(), error.code()) {
        (ErrorCategory::Unauthorised | ErrorCategory::Forbidden, _) => StatusCode::UNAUTHORIZED,
        (_, ErrorCode::NotFound | ErrorCode::InvalidValue) => StatusCode::NOT_FOUND,
        (ErrorCategory::Conflict, _) => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    no_store((status, Html(String::new())).into_response())
}

fn render_html<T: Template>(status: StatusCode, template: &T) -> Response {
    let Ok(rendered) = template.render() else {
        return no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };
    no_store((status, Html(rendered)).into_response())
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}
