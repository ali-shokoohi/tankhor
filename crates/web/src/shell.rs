//! Bilingual Askama base shell, field partial, and local static asset routes.

use std::sync::Arc;

use askama::Template;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use kernel::locale::{Direction, Locale, Theme};
use serde::Deserialize;

use crate::GENERATED_TOKENS_CSS;
use crate::i18n::{ConsoleCatalog, I18nError};

/// Embedded logical-property console stylesheet.
pub const CONSOLE_STYLESHEET: &str = include_str!("../static/console.css");

/// Self-hosted Vazirmatn variable WOFF2 binary.
pub const VAZIRMATN_WOFF2: &[u8] = include_bytes!("../static/vendor/vazirmatn.woff2");

/// Bundled SIL Open Font License 1.1 text for Vazirmatn.
pub const VAZIRMATN_LICENSE: &str = include_str!("../static/vendor/OFL.txt");

/// Self-hosted vendored htmx 2.x bundle.
pub const HTMX_MIN_JS: &str = include_str!("../static/vendor/htmx.min.js");

/// Bundled license text for htmx.
pub const HTMX_LICENSE: &str = include_str!("../static/vendor/htmx-LICENSE.txt");

/// Presentation preferences bound to the rendered console shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShellPreferences {
    locale: Locale,
    theme: Theme,
}

impl ShellPreferences {
    /// Construct preferences with derived direction from `locale`.
    #[must_use]
    pub const fn new(locale: Locale, theme: Theme) -> Self {
        Self { locale, theme }
    }

    /// Selected display locale (`fa` or `en`).
    #[must_use]
    pub const fn locale(self) -> Locale {
        self.locale
    }

    /// Writing direction derived from `locale`.
    #[must_use]
    pub const fn direction(self) -> Direction {
        self.locale.direction()
    }

    /// Selected theme (`light`, `dark`, or `system`).
    #[must_use]
    pub const fn theme(self) -> Theme {
        self.theme
    }
}

/// Reusable form-field view model rendered by `partials/field.html`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldViewModel {
    /// Unique DOM id shared between `<label for>` and `<input id>`.
    pub id: String,
    /// Form control `name` attribute.
    pub name: String,
    /// Localized accessible label text.
    pub label: String,
    /// Safe preserved input value.
    pub value: String,
    /// Optional localized validation message associated via `aria-describedby`.
    pub error: Option<String>,
    /// Whether the field carries `required`.
    pub required: bool,
}

/// Askama template view for the foundational console layout.
#[derive(Debug, Clone, Template)]
#[template(path = "console/layout.html")]
#[allow(clippy::struct_excessive_bools)]
pub struct ShellView {
    html_lang: &'static str,
    html_dir: &'static str,
    data_theme: &'static str,
    is_locale_fa: bool,
    is_locale_en: bool,
    is_theme_light: bool,
    is_theme_dark: bool,
    is_theme_system: bool,
    brand_name: String,
    page_title: String,
    heading: String,
    status_message: String,
    skip_to_main: String,
    nav_primary_label: String,
    preferences_form_label: String,
    language_label: String,
    language_fa: String,
    language_en: String,
    theme_label: String,
    theme_light: String,
    theme_dark: String,
    theme_system: String,
    preferences_apply: String,
    csrf_token: String,
    sample_field: Option<FieldViewModel>,
}

impl ShellView {
    /// Populate the foundational console shell strictly from [`ConsoleCatalog`] for `preferences`.
    ///
    /// # Errors
    ///
    /// Returns [`I18nError`] if any required foundation message key fails to format.
    pub fn foundation(
        catalog: &ConsoleCatalog,
        preferences: ShellPreferences,
        csrf_token: &str,
        sample_field: Option<FieldViewModel>,
    ) -> Result<Self, I18nError> {
        let locale = preferences.locale();
        let theme = preferences.theme();

        Ok(Self {
            html_lang: locale.as_tag(),
            html_dir: preferences.direction().as_str(),
            data_theme: theme.as_str(),
            is_locale_fa: locale == Locale::Fa,
            is_locale_en: locale == Locale::En,
            is_theme_light: theme == Theme::Light,
            is_theme_dark: theme == Theme::Dark,
            is_theme_system: theme == Theme::System,
            brand_name: catalog.format(locale, "brand-name", &[])?,
            page_title: catalog.format(locale, "console-title", &[])?,
            heading: catalog.format(locale, "console-foundation-heading", &[])?,
            status_message: catalog.format(locale, "console-foundation-status", &[])?,
            skip_to_main: catalog.format(locale, "skip-to-main", &[])?,
            nav_primary_label: catalog.format(locale, "nav-primary-label", &[])?,
            preferences_form_label: catalog.format(locale, "preferences-form-label", &[])?,
            language_label: catalog.format(locale, "language-label", &[])?,
            language_fa: catalog.format(locale, "language-fa", &[])?,
            language_en: catalog.format(locale, "language-en", &[])?,
            theme_label: catalog.format(locale, "theme-label", &[])?,
            theme_light: catalog.format(locale, "theme-light", &[])?,
            theme_dark: catalog.format(locale, "theme-dark", &[])?,
            theme_system: catalog.format(locale, "theme-system", &[])?,
            preferences_apply: catalog.format(locale, "preferences-apply", &[])?,
            csrf_token: csrf_token.to_owned(),
            sample_field,
        })
    }

    /// Render the Askama layout to a full HTML document string.
    ///
    /// # Errors
    ///
    /// Returns [`askama::Error`] if template rendering fails.
    pub fn render_html(&self) -> Result<String, askama::Error> {
        self.render()
    }
}

#[derive(Debug, Default, Deserialize)]
struct ShellQuery {
    lang: Option<String>,
    theme: Option<String>,
}

/// Build the foundational console shell and local static-asset router.
///
/// No placeholder protected management or sign-in routes are registered here.
pub fn shell_router(catalog: ConsoleCatalog) -> Router {
    let shared_catalog = Arc::new(catalog);
    Router::new()
        .route("/console", get(handle_console_shell))
        .route("/static/tokens.css", get(handle_tokens_css))
        .route("/static/console.css", get(handle_console_css))
        .route(
            "/static/vendor/vazirmatn.woff2",
            get(handle_vazirmatn_woff2),
        )
        .route("/static/vendor/OFL.txt", get(handle_vazirmatn_license))
        .route("/static/vendor/htmx.min.js", get(handle_htmx_js))
        .route("/static/vendor/htmx-LICENSE.txt", get(handle_htmx_license))
        .with_state(shared_catalog)
}

async fn handle_console_shell(
    State(catalog): State<Arc<ConsoleCatalog>>,
    Query(query): Query<ShellQuery>,
) -> Response {
    let locale = query
        .lang
        .as_deref()
        .and_then(|v| v.parse::<Locale>().ok())
        .unwrap_or_default();
    let theme = query
        .theme
        .as_deref()
        .and_then(|v| v.parse::<Theme>().ok())
        .unwrap_or_default();

    let prefs = ShellPreferences::new(locale, theme);
    match ShellView::foundation(&catalog, prefs, "", None)
        .ok()
        .and_then(|view| view.render_html().ok())
    {
        Some(rendered) => Html(rendered).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn handle_tokens_css() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        GENERATED_TOKENS_CSS,
    )
        .into_response()
}

async fn handle_console_css() -> Response {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        CONSOLE_STYLESHEET,
    )
        .into_response()
}

async fn handle_vazirmatn_woff2() -> Response {
    ([(header::CONTENT_TYPE, "font/woff2")], VAZIRMATN_WOFF2).into_response()
}

async fn handle_vazirmatn_license() -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        VAZIRMATN_LICENSE,
    )
        .into_response()
}

async fn handle_htmx_js() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        HTMX_MIN_JS,
    )
        .into_response()
}

async fn handle_htmx_license() -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        HTMX_LICENSE,
    )
        .into_response()
}
