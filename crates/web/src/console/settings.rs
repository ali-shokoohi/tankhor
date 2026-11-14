//! Settings registry list, detail, and draft form routes.
//! FR-006, FR-007, FR-008, FR-016, FR-CON-32, FR-UX-08, AC-ADM-008/009/010/015/037/038.

use std::{str::FromStr, sync::Arc};

use admin::{
    application::{
        console_services::ConsoleServices,
        get_setting::{GetSetting, GetSettingInput},
        list_settings::{ListSettings, ListSettingsInput, SettingSummary, SettingVersionView},
        save_draft::{SaveDraft, SaveDraftInput, ScheduleInput},
    },
    domain::{
        audit::ReasonCode,
        auth_state::BindingDigest,
        change_revision::{ChangeSubject, ChangeValue},
        formats::{FormatSet, ImageFormat},
        secrets::SecretBytes,
        settings::{SettingKey, SettingUnit, SettingValue},
    },
    ports::admin_authenticator::TokenPurpose,
};
use askama::Template;
use axum::{
    Router,
    body::Bytes,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{ChangeId, RevisionId},
    locale::Locale,
};
use serde::Deserialize;

use crate::{
    i18n::ConsoleCatalog,
    security::{
        CookieKind, CookieSecurityPolicy, CsrfContext, CsrfProtectionState, enforce_csrf_middleware,
    },
};

#[derive(Clone)]
struct SettingsState {
    services: ConsoleServices,
    catalog: Arc<ConsoleCatalog>,
    cookies: CookieSecurityPolicy,
}

#[derive(Deserialize, Default)]
struct LocaleQuery {
    lang: Option<String>,
}

#[derive(Clone)]
struct SettingRowView {
    key: String,
    label: String,
    value: String,
    minimum: u32,
    maximum: u32,
    unit: String,
}

#[derive(Clone)]
struct FormatOptionView {
    code: &'static str,
    checked: bool,
}

#[derive(Clone)]
struct SettingHistoryRowView {
    version: String,
    value: String,
    actor: String,
    effective_at: String,
}

#[derive(Template)]
#[template(path = "console/settings.html")]
struct SettingsListTemplate {
    lang: String,
    dir: String,
    heading: String,
    rows: Vec<SettingRowView>,
}

#[derive(Template)]
#[template(path = "console/setting.html")]
struct SettingDetailTemplate {
    lang: String,
    dir: String,
    heading: String,
    key: String,
    label: String,
    live_value_label: String,
    live_value: String,
    bounds_label: String,
    minimum: u32,
    maximum: u32,
    unit_label: String,
    unit: String,
    live_version: String,
    draft_heading: String,
    csrf: Option<String>,
    change_id: Option<String>,
    expected_head: Option<String>,
    is_format_setting: bool,
    formats_label: String,
    format_options: Vec<FormatOptionView>,
    value_label: String,
    proposed_value: String,
    value_error: Option<String>,
    conflict_error: Option<String>,
    schedule_date_label: String,
    schedule_time_label: String,
    schedule_tz_label: String,
    local_date: String,
    local_time: String,
    time_zone: String,
    schedule_error: Option<String>,
    draft_submit: String,
    history_heading: String,
    history: Vec<SettingHistoryRowView>,
}

/// Protected `GET /console/settings` router.
///
/// # Panics
/// Panics if the embedded authoritative Fluent catalog fails to load.
pub fn settings_list_router(services: ConsoleServices, cookies: CookieSecurityPolicy) -> Router {
    let catalog = Arc::new(ConsoleCatalog::load_authoritative().expect("console catalog"));
    Router::new()
        .route("/console/settings", get(get_settings))
        .with_state(SettingsState {
            services,
            catalog,
            cookies,
        })
}

/// Protected `GET /console/settings/{key}` router.
///
/// # Panics
/// Panics if the embedded authoritative Fluent catalog fails to load.
pub fn setting_detail_router(services: ConsoleServices, cookies: CookieSecurityPolicy) -> Router {
    let catalog = Arc::new(ConsoleCatalog::load_authoritative().expect("console catalog"));
    Router::new()
        .route("/console/settings/{key}", get(get_setting))
        .with_state(SettingsState {
            services,
            catalog,
            cookies,
        })
}

/// Protected `POST /console/settings/{key}/drafts` router.
///
/// # Panics
/// Panics if the embedded authoritative Fluent catalog fails to load.
pub fn setting_draft_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    let catalog = Arc::new(ConsoleCatalog::load_authoritative().expect("console catalog"));
    Router::new()
        .route("/console/settings/{key}/drafts", post(post_setting_draft))
        .route_layer(middleware::from_fn_with_state(
            csrf,
            enforce_csrf_middleware,
        ))
        .with_state(SettingsState {
            services,
            catalog,
            cookies,
        })
}

/// Combined protected settings router (`GET /console/settings`, `GET /console/settings/{key}`, `POST /console/settings/{key}/drafts`).
pub fn settings_router(
    services: ConsoleServices,
    csrf: CsrfProtectionState,
    cookies: CookieSecurityPolicy,
) -> Router {
    settings_list_router(services.clone(), cookies)
        .merge(setting_detail_router(services.clone(), cookies))
        .merge(setting_draft_router(services, csrf, cookies))
}

async fn get_settings(
    State(state): State<SettingsState>,
    headers: HeaderMap,
    Query(query): Query<LocaleQuery>,
) -> Response {
    let locale = parse_locale(query.lang.as_deref());
    let Some(actor) = session_digest(&state, &headers).await else {
        return no_store(StatusCode::UNAUTHORIZED.into_response());
    };
    let service = ListSettings {
        uow: state.services.uow.as_ref(),
        admins: state.services.admins.as_ref(),
        sessions: state.services.sessions.as_ref(),
        settings: state.services.settings.as_ref(),
        clock: state.services.clock.as_ref(),
    };
    let Ok(list) = service.execute(ListSettingsInput { actor }).await else {
        return no_store(StatusCode::UNAUTHORIZED.into_response());
    };
    let rows = list
        .entries
        .iter()
        .map(|entry| SettingRowView {
            key: entry.key.as_str().to_owned(),
            label: msg(&state.catalog, locale, &entry.label_key),
            value: format_setting_value(&entry.value),
            minimum: entry.minimum,
            maximum: entry.maximum,
            unit: msg(&state.catalog, locale, unit_message_key(entry.unit)),
        })
        .collect();
    let html = SettingsListTemplate {
        lang: locale.as_tag().to_owned(),
        dir: locale.direction().as_str().to_owned(),
        heading: msg(&state.catalog, locale, "settings-heading"),
        rows,
    }
    .render()
    .expect("settings list template");
    no_store((StatusCode::OK, Html(html)).into_response())
}

async fn get_setting(
    State(state): State<SettingsState>,
    Path(key): Path<String>,
    headers: HeaderMap,
    csrf_ext: Option<Extension<CsrfContext>>,
    Query(query): Query<LocaleQuery>,
) -> Response {
    let locale = parse_locale(query.lang.as_deref());
    let Some(actor) = session_digest(&state, &headers).await else {
        return no_store(StatusCode::UNAUTHORIZED.into_response());
    };
    let csrf = csrf_token(csrf_ext);
    let service = GetSetting {
        uow: state.services.uow.as_ref(),
        admins: state.services.admins.as_ref(),
        sessions: state.services.sessions.as_ref(),
        settings: state.services.settings.as_ref(),
        clock: state.services.clock.as_ref(),
    };
    match service.execute(GetSettingInput { actor, key }).await {
        Ok(summary) => {
            let initial_proposed = match &summary.value {
                SettingValue::Number(n) => n.to_string(),
                SettingValue::Boolean(b) => b.to_string(),
                SettingValue::Formats(_) => String::new(),
            };
            let initial_formats = match &summary.value {
                SettingValue::Formats(formats) => format_options_from_set(*formats),
                SettingValue::Number(_) | SettingValue::Boolean(_) => default_format_options(),
            };
            let html = render_setting_detail(
                &state.catalog,
                locale,
                &summary,
                csrf,
                DraftFormPreserve {
                    change_id: None,
                    expected_head: None,
                    proposed_value: initial_proposed,
                    format_options: initial_formats,
                    value_error: None,
                    conflict_error: None,
                    local_date: String::new(),
                    local_time: String::new(),
                    time_zone: String::new(),
                    schedule_error: None,
                },
            );
            no_store((StatusCode::OK, Html(html)).into_response())
        }
        Err(err) if err.code() == ErrorCode::AuthenticationRequired => {
            no_store(StatusCode::UNAUTHORIZED.into_response())
        }
        Err(err) if matches!(err.code(), ErrorCode::NotFound | ErrorCode::InvalidValue) => {
            no_store(StatusCode::NOT_FOUND.into_response())
        }
        Err(_) => no_store(StatusCode::SERVICE_UNAVAILABLE.into_response()),
    }
}

#[derive(Default)]
struct ParsedDraftForm {
    lang: Option<String>,
    change: Option<String>,
    expected_head: Option<String>,
    value: Option<String>,
    formats: Vec<String>,
    local_date: Option<String>,
    local_time: Option<String>,
    time_zone: Option<String>,
}

struct DraftFormPreserve {
    change_id: Option<String>,
    expected_head: Option<String>,
    proposed_value: String,
    format_options: Vec<FormatOptionView>,
    value_error: Option<String>,
    conflict_error: Option<String>,
    local_date: String,
    local_time: String,
    time_zone: String,
    schedule_error: Option<String>,
}

#[allow(clippy::too_many_lines)]
async fn post_setting_draft(
    State(state): State<SettingsState>,
    Path(key): Path<String>,
    headers: HeaderMap,
    csrf_ext: Option<Extension<CsrfContext>>,
    Query(query): Query<LocaleQuery>,
    body: Bytes,
) -> Response {
    let form = parse_draft_form(&body);
    let locale = parse_locale(form.lang.as_deref().or(query.lang.as_deref()));
    let Some(actor) = session_digest(&state, &headers).await else {
        return no_store(StatusCode::UNAUTHORIZED.into_response());
    };
    let csrf = csrf_token(csrf_ext);

    let get_service = GetSetting {
        uow: state.services.uow.as_ref(),
        admins: state.services.admins.as_ref(),
        sessions: state.services.sessions.as_ref(),
        settings: state.services.settings.as_ref(),
        clock: state.services.clock.as_ref(),
    };
    let summary = match get_service
        .execute(GetSettingInput {
            actor: actor.clone(),
            key,
        })
        .await
    {
        Ok(summary) => summary,
        Err(err) if err.code() == ErrorCode::AuthenticationRequired => {
            return no_store(StatusCode::UNAUTHORIZED.into_response());
        }
        Err(err) if matches!(err.code(), ErrorCode::NotFound | ErrorCode::InvalidValue) => {
            return no_store(StatusCode::NOT_FOUND.into_response());
        }
        Err(_) => return no_store(StatusCode::SERVICE_UNAVAILABLE.into_response()),
    };

    let preserved_change_id = non_empty(form.change.as_deref());
    let preserved_expected_head = non_empty(form.expected_head.as_deref());
    let preserved_value = form.value.clone().unwrap_or_default();
    let preserved_local_date = form.local_date.clone().unwrap_or_default();
    let preserved_local_time = form.local_time.clone().unwrap_or_default();
    let preserved_time_zone = form.time_zone.clone().unwrap_or_default();
    let preserved_format_options =
        format_options_from_strings(&form.formats, form.value.as_deref());

    let Ok(parsed_change_id) = parse_optional_change_id(preserved_change_id.as_deref()) else {
        return render_error_response(
            &state.catalog,
            locale,
            &summary,
            csrf,
            StatusCode::UNPROCESSABLE_ENTITY,
            DraftFormPreserve {
                change_id: preserved_change_id,
                expected_head: preserved_expected_head,
                proposed_value: preserved_value,
                format_options: preserved_format_options,
                value_error: Some(bounds_error_message(&state.catalog, locale, &summary)),
                conflict_error: None,
                local_date: preserved_local_date,
                local_time: preserved_local_time,
                time_zone: preserved_time_zone,
                schedule_error: None,
            },
        );
    };

    let Ok(parsed_expected_head) = parse_optional_revision_id(preserved_expected_head.as_deref())
    else {
        return render_error_response(
            &state.catalog,
            locale,
            &summary,
            csrf,
            StatusCode::CONFLICT,
            DraftFormPreserve {
                change_id: preserved_change_id,
                expected_head: preserved_expected_head,
                proposed_value: preserved_value,
                format_options: preserved_format_options,
                value_error: None,
                conflict_error: Some(msg(&state.catalog, locale, "setting-draft-conflict")),
                local_date: preserved_local_date,
                local_time: preserved_local_time,
                time_zone: preserved_time_zone,
                schedule_error: None,
            },
        );
    };

    let Ok(parsed_schedule) = parse_optional_schedule(
        &preserved_local_date,
        &preserved_local_time,
        &preserved_time_zone,
    ) else {
        return render_error_response(
            &state.catalog,
            locale,
            &summary,
            csrf,
            StatusCode::UNPROCESSABLE_ENTITY,
            DraftFormPreserve {
                change_id: preserved_change_id,
                expected_head: preserved_expected_head,
                proposed_value: preserved_value,
                format_options: preserved_format_options,
                value_error: None,
                conflict_error: None,
                local_date: preserved_local_date,
                local_time: preserved_local_time,
                time_zone: preserved_time_zone,
                schedule_error: Some(msg(&state.catalog, locale, "setting-schedule-invalid")),
            },
        );
    };

    let Ok(parsed_change_value) =
        parse_proposed_change_value(summary.key, &preserved_value, &form.formats)
    else {
        return render_error_response(
            &state.catalog,
            locale,
            &summary,
            csrf,
            StatusCode::UNPROCESSABLE_ENTITY,
            DraftFormPreserve {
                change_id: preserved_change_id,
                expected_head: preserved_expected_head,
                proposed_value: preserved_value,
                format_options: preserved_format_options,
                value_error: Some(bounds_error_message(&state.catalog, locale, &summary)),
                conflict_error: None,
                local_date: preserved_local_date,
                local_time: preserved_local_time,
                time_zone: preserved_time_zone,
                schedule_error: None,
            },
        );
    };

    let save_service = SaveDraft {
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

    let schedule_requested = parsed_schedule.is_some();
    match save_service
        .execute(SaveDraftInput {
            actor,
            change: parsed_change_id,
            subject: ChangeSubject::Setting(summary.key),
            value: parsed_change_value,
            reason: ReasonCode::DraftChange,
            expected_head: parsed_expected_head,
            schedule: parsed_schedule,
        })
        .await
    {
        Ok(draft) => {
            let location = format!("/console/changes/{}", draft.id());
            redirect_303(&location)
        }
        Err(err) if err.code() == ErrorCode::AuthenticationRequired => {
            no_store(StatusCode::UNAUTHORIZED.into_response())
        }
        Err(err) if err.code() == ErrorCode::StaleRevision => render_error_response(
            &state.catalog,
            locale,
            &summary,
            csrf,
            StatusCode::CONFLICT,
            DraftFormPreserve {
                change_id: preserved_change_id,
                expected_head: preserved_expected_head,
                proposed_value: preserved_value,
                format_options: preserved_format_options,
                value_error: None,
                conflict_error: Some(msg(&state.catalog, locale, "setting-draft-conflict")),
                local_date: preserved_local_date,
                local_time: preserved_local_time,
                time_zone: preserved_time_zone,
                schedule_error: None,
            },
        ),
        Err(err) if err.code() == ErrorCode::InvalidValue => {
            let is_value_in_bounds = is_numeric_within_bounds(
                summary.key,
                &preserved_value,
                summary.minimum,
                summary.maximum,
            );
            let (value_error, schedule_error) = if schedule_requested && is_value_in_bounds {
                (
                    None,
                    Some(msg(&state.catalog, locale, "setting-schedule-invalid")),
                )
            } else {
                (
                    Some(bounds_error_message(&state.catalog, locale, &summary)),
                    None,
                )
            };
            render_error_response(
                &state.catalog,
                locale,
                &summary,
                csrf,
                StatusCode::UNPROCESSABLE_ENTITY,
                DraftFormPreserve {
                    change_id: preserved_change_id,
                    expected_head: preserved_expected_head,
                    proposed_value: preserved_value,
                    format_options: preserved_format_options,
                    value_error,
                    conflict_error: None,
                    local_date: preserved_local_date,
                    local_time: preserved_local_time,
                    time_zone: preserved_time_zone,
                    schedule_error,
                },
            )
        }
        Err(err) if err.code() == ErrorCode::NotFound => {
            no_store(StatusCode::NOT_FOUND.into_response())
        }
        Err(_) => no_store(StatusCode::SERVICE_UNAVAILABLE.into_response()),
    }
}

fn render_error_response(
    catalog: &ConsoleCatalog,
    locale: Locale,
    summary: &SettingSummary,
    csrf: Option<String>,
    status: StatusCode,
    preserve: DraftFormPreserve,
) -> Response {
    let html = render_setting_detail(catalog, locale, summary, csrf, preserve);
    no_store((status, Html(html)).into_response())
}

fn render_setting_detail(
    catalog: &ConsoleCatalog,
    locale: Locale,
    summary: &SettingSummary,
    csrf: Option<String>,
    preserve: DraftFormPreserve,
) -> String {
    let history = summary
        .history
        .iter()
        .map(format_history_row)
        .collect::<Vec<_>>();
    SettingDetailTemplate {
        lang: locale.as_tag().to_owned(),
        dir: locale.direction().as_str().to_owned(),
        heading: msg(catalog, locale, "setting-detail-heading"),
        key: summary.key.as_str().to_owned(),
        label: msg(catalog, locale, &summary.label_key),
        live_value_label: msg(catalog, locale, "setting-live-value-label"),
        live_value: format_setting_value(&summary.value),
        bounds_label: msg(catalog, locale, "setting-bounds-label"),
        minimum: summary.minimum,
        maximum: summary.maximum,
        unit_label: msg(catalog, locale, "setting-unit-label"),
        unit: msg(catalog, locale, unit_message_key(summary.unit)),
        live_version: summary.live_version.to_string(),
        draft_heading: msg(catalog, locale, "setting-draft-heading"),
        csrf,
        change_id: preserve.change_id,
        expected_head: preserve.expected_head,
        is_format_setting: summary.key == SettingKey::UploadFormats,
        formats_label: msg(catalog, locale, "setting-formats-label"),
        format_options: preserve.format_options,
        value_label: msg(catalog, locale, "setting-value-label"),
        proposed_value: preserve.proposed_value,
        value_error: preserve.value_error,
        conflict_error: preserve.conflict_error,
        schedule_date_label: msg(catalog, locale, "setting-schedule-date-label"),
        schedule_time_label: msg(catalog, locale, "setting-schedule-time-label"),
        schedule_tz_label: msg(catalog, locale, "setting-schedule-tz-label"),
        local_date: preserve.local_date,
        local_time: preserve.local_time,
        time_zone: preserve.time_zone,
        schedule_error: preserve.schedule_error,
        draft_submit: msg(catalog, locale, "setting-draft-submit"),
        history_heading: msg(catalog, locale, "setting-history-heading"),
        history,
    }
    .render()
    .expect("setting detail template")
}

fn format_history_row(view: &SettingVersionView) -> SettingHistoryRowView {
    SettingHistoryRowView {
        version: view.version.to_string(),
        value: format_setting_value(&view.value),
        actor: view.actor.to_string(),
        effective_at: view.effective_at.to_string(),
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

fn default_format_options() -> Vec<FormatOptionView> {
    [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::WebP,
        ImageFormat::Heic,
    ]
    .into_iter()
    .map(|format| FormatOptionView {
        code: format.as_str(),
        checked: false,
    })
    .collect()
}

fn format_options_from_set(set: FormatSet) -> Vec<FormatOptionView> {
    [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::WebP,
        ImageFormat::Heic,
    ]
    .into_iter()
    .map(|format| FormatOptionView {
        code: format.as_str(),
        checked: set.contains(format),
    })
    .collect()
}

fn format_options_from_strings(
    formats: &[String],
    fallback_value: Option<&str>,
) -> Vec<FormatOptionView> {
    let mut selected: Vec<String> = formats
        .iter()
        .flat_map(|item| item.split(','))
        .map(|s| s.trim().to_ascii_uppercase())
        .filter(|s| !s.is_empty())
        .collect();
    if selected.is_empty()
        && let Some(raw) = fallback_value
    {
        if let Ok(mask) = raw.trim().parse::<u8>() {
            return [
                ImageFormat::Jpeg,
                ImageFormat::Png,
                ImageFormat::WebP,
                ImageFormat::Heic,
            ]
            .into_iter()
            .map(|format| FormatOptionView {
                code: format.as_str(),
                checked: mask & format.bit() != 0,
            })
            .collect();
        }
        selected.extend(
            raw.split(',')
                .map(|s| s.trim().to_ascii_uppercase())
                .filter(|s| !s.is_empty()),
        );
    }
    [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::WebP,
        ImageFormat::Heic,
    ]
    .into_iter()
    .map(|format| FormatOptionView {
        code: format.as_str(),
        checked: selected.iter().any(|code| code == format.as_str()),
    })
    .collect()
}

fn parse_proposed_change_value(
    key: SettingKey,
    raw_value: &str,
    formats: &[String],
) -> Result<ChangeValue, ()> {
    if key == SettingKey::UploadFormats {
        if !formats.is_empty() {
            let joined = formats.join(",");
            let set = FormatSet::from_str(&joined).map_err(|_| ())?;
            return Ok(ChangeValue::Formats(set));
        }
        let trimmed = raw_value.trim();
        if trimmed.is_empty() {
            return Err(());
        }
        if let Ok(mask) = trimmed.parse::<u8>() {
            let set = FormatSet::from_mask(mask).map_err(|_| ())?;
            return Ok(ChangeValue::Formats(set));
        }
        let set = FormatSet::from_str(trimmed).map_err(|_| ())?;
        return Ok(ChangeValue::Formats(set));
    }
    let number = raw_value.trim().parse::<u32>().map_err(|_| ())?;
    Ok(ChangeValue::Number(number))
}

fn is_numeric_within_bounds(key: SettingKey, raw_value: &str, min: u32, max: u32) -> bool {
    if key == SettingKey::UploadFormats {
        return false;
    }
    let Ok(val) = raw_value.trim().parse::<u32>() else {
        return false;
    };
    val >= min && val <= max
}

fn bounds_error_message(
    catalog: &ConsoleCatalog,
    locale: Locale,
    summary: &SettingSummary,
) -> String {
    if summary.key == SettingKey::UploadFormats {
        return msg(catalog, locale, "setting-formats-jpeg-required");
    }
    let minimum = summary.minimum.to_string();
    let maximum = summary.maximum.to_string();
    catalog
        .format(
            locale,
            "setting-value-out-of-bounds",
            &[("minimum", minimum.as_str()), ("maximum", maximum.as_str())],
        )
        .expect("bounds message")
}

const fn unit_message_key(unit: SettingUnit) -> &'static str {
    match unit {
        SettingUnit::DaysFromOriginalUpload => "setting-unit-days_from_original_upload",
        SettingUnit::DaysFromLastActivity => "setting-unit-days_from_last_activity",
        SettingUnit::DaysFromDeletionRequest => "setting-unit-days_from_deletion_request",
        SettingUnit::Cards => "setting-unit-cards",
        SettingUnit::Photos => "setting-unit-photos",
        SettingUnit::Megabytes => "setting-unit-megabytes",
        SettingUnit::Pixels => "setting-unit-pixels",
        SettingUnit::TotalAttempts => "setting-unit-total_attempts",
        SettingUnit::FormatMask => "setting-unit-format_mask",
    }
}

fn parse_optional_change_id(raw: Option<&str>) -> Result<Option<ChangeId>, DomainError> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let id =
        ChangeId::from_str(value).map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
    Ok(Some(id))
}

fn parse_optional_revision_id(raw: Option<&str>) -> Result<Option<RevisionId>, DomainError> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let id =
        RevisionId::from_str(value).map_err(|_| DomainError::conflict(ErrorCode::StaleRevision))?;
    Ok(Some(id))
}

fn parse_optional_schedule(
    local_date: &str,
    local_time: &str,
    time_zone: &str,
) -> Result<Option<ScheduleInput>, ()> {
    let date = local_date.trim();
    let time = local_time.trim();
    let tz = time_zone.trim();
    if date.is_empty() && time.is_empty() && tz.is_empty() {
        return Ok(None);
    }
    if date.is_empty() || time.is_empty() {
        return Err(());
    }
    Ok(Some(ScheduleInput {
        local: format!("{date}T{time}"),
        zone: (!tz.is_empty()).then(|| tz.to_owned()),
    }))
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_draft_form(body: &Bytes) -> ParsedDraftForm {
    let mut parsed = ParsedDraftForm::default();
    let Ok(text) = std::str::from_utf8(body) else {
        return parsed;
    };
    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        let Ok(key) = percent_decode(raw_key) else {
            continue;
        };
        let Ok(value) = percent_decode(raw_value) else {
            continue;
        };
        match key.as_str() {
            "lang" => parsed.lang = Some(value),
            "change" => parsed.change = Some(value),
            "expected_head" => parsed.expected_head = Some(value),
            "value" => parsed.value = Some(value),
            "formats" => parsed.formats.push(value),
            "local_date" => parsed.local_date = Some(value),
            "local_time" => parsed.local_time = Some(value),
            "time_zone" => parsed.time_zone = Some(value),
            _ => {}
        }
    }
    parsed
}

fn percent_decode(input: &str) -> Result<String, ()> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(());
                }
                let hi = hex_nibble(bytes[index + 1]).ok_or(())?;
                let lo = hex_nibble(bytes[index + 2]).ok_or(())?;
                out.push((hi << 4) | lo);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_locale(lang: Option<&str>) -> Locale {
    lang.and_then(|value| Locale::from_str(value).ok())
        .unwrap_or_default()
}

fn msg(catalog: &ConsoleCatalog, locale: Locale, key: &str) -> String {
    catalog.format(locale, key, &[]).expect("message")
}

async fn session_digest(state: &SettingsState, headers: &HeaderMap) -> Option<BindingDigest> {
    let token = super::auth::cookie(headers, CookieKind::AdminSession.cookie_name(state.cookies))?;
    state
        .services
        .authenticator
        .token_digest(
            TokenPurpose::Session,
            SecretBytes::new(token.as_bytes().to_vec()),
        )
        .await
        .ok()
}

fn csrf_token(csrf_ext: Option<Extension<CsrfContext>>) -> Option<String> {
    csrf_ext
        .and_then(|Extension(context)| String::from_utf8(context.expected_bytes().to_vec()).ok())
}

fn redirect_303(location: &str) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    response
        .headers_mut()
        .insert(header::LOCATION, HeaderValue::from_str(location).unwrap());
    no_store(response)
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
