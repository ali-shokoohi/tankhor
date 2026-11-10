//! Bilingual Fluent localization catalog (`fa` / `en`) with compile/load-time key,
//! placeable-variable, and `.surface` / `.situation` context parity enforcement.

use std::collections::{BTreeMap, BTreeSet};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use kernel::locale::Locale;
use thiserror::Error;
use unic_langid::LanguageIdentifier;

const AUTHORITATIVE_FA_FTL: &str = include_str!("../locales/fa/console.ftl");
const AUTHORITATIVE_EN_FTL: &str = include_str!("../locales/en/console.ftl");

const ALLOWED_SURFACES: &[&str] = &["console"];
const ALLOWED_SITUATIONS: &[&str] = &[
    "shell",
    "accessibility",
    "theme",
    "validation",
    "security",
    "transport",
];

/// Context metadata attached to every Fluent message via `.surface` and `.situation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageMetadata {
    surface: String,
    situation: String,
    variables: BTreeSet<String>,
}

impl MessageMetadata {
    #[must_use]
    pub fn surface(&self) -> &str {
        &self.surface
    }

    #[must_use]
    pub fn situation(&self) -> &str {
        &self.situation
    }

    #[must_use]
    pub fn variables(&self) -> &BTreeSet<String> {
        &self.variables
    }
}

/// Classification of localization catalog and formatting failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum I18nErrorKind {
    SyntaxError,
    DuplicateMessage,
    MissingContextMetadata,
    KeyParityMismatch,
    VariableParityMismatch,
    ContextParityMismatch,
    ForbiddenBrandingOrEmoji,
    MissingMessage,
    FormattingFailed,
}

/// Localization error with explicit category and diagnostic detail.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{kind:?}: {detail}")]
pub struct I18nError {
    kind: I18nErrorKind,
    detail: String,
}

impl I18nError {
    fn new(kind: I18nErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> I18nErrorKind {
        self.kind
    }

    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Thread-safe bilingual Fluent catalog for the admin console (`Locale::Fa` and `Locale::En`).
///
/// Never falls back silently across locales: missing keys or mismatched variables fail explicitly.
pub struct ConsoleCatalog {
    fa_bundle: FluentBundle<FluentResource>,
    en_bundle: FluentBundle<FluentResource>,
    fa_meta: BTreeMap<String, MessageMetadata>,
    en_meta: BTreeMap<String, MessageMetadata>,
}

impl std::fmt::Debug for ConsoleCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConsoleCatalog")
            .field("message_count", &self.fa_meta.len())
            .finish_non_exhaustive()
    }
}

impl ConsoleCatalog {
    /// Loads the authoritative `crates/web/locales/{fa,en}/console.ftl` catalog.
    ///
    /// # Errors
    ///
    /// Returns [`I18nError`] if syntax, context metadata, or FA/EN parity validation fails.
    pub fn load_authoritative() -> Result<Self, I18nError> {
        Self::from_sources(AUTHORITATIVE_FA_FTL, AUTHORITATIVE_EN_FTL)
    }

    /// Parses and validates a pair of Persian and English Fluent sources.
    ///
    /// # Errors
    ///
    /// Returns [`I18nError`] on syntax errors, missing `.surface`/`.situation` attributes,
    /// mismatched message keys, mismatched placeable variables, or forbidden copy tokens.
    pub fn from_sources(fa_src: &str, en_src: &str) -> Result<Self, I18nError> {
        let fa_meta = inspect_resource("fa", fa_src)?;
        let en_meta = inspect_resource("en", en_src)?;

        let fa_keys: BTreeSet<&String> = fa_meta.keys().collect();
        let en_keys: BTreeSet<&String> = en_meta.keys().collect();
        if fa_keys != en_keys {
            let only_fa: Vec<&str> = fa_keys.difference(&en_keys).map(|k| k.as_str()).collect();
            let only_en: Vec<&str> = en_keys.difference(&fa_keys).map(|k| k.as_str()).collect();
            return Err(I18nError::new(
                I18nErrorKind::KeyParityMismatch,
                format!("FA/EN key mismatch (only_fa={only_fa:?}, only_en={only_en:?})"),
            ));
        }

        for (key, fa_entry) in &fa_meta {
            let en_entry = &en_meta[key];
            if fa_entry.variables != en_entry.variables {
                return Err(I18nError::new(
                    I18nErrorKind::VariableParityMismatch,
                    format!(
                        "message `{key}` placeable mismatch: fa={:?}, en={:?}",
                        fa_entry.variables, en_entry.variables
                    ),
                ));
            }
            if fa_entry.surface != en_entry.surface || fa_entry.situation != en_entry.situation {
                return Err(I18nError::new(
                    I18nErrorKind::ContextParityMismatch,
                    format!("message `{key}` context metadata mismatch between FA and EN"),
                ));
            }
        }

        let fa_lang: LanguageIdentifier = "fa".parse().map_err(|err| {
            I18nError::new(
                I18nErrorKind::SyntaxError,
                format!("invalid fa langid: {err}"),
            )
        })?;
        let en_lang: LanguageIdentifier = "en".parse().map_err(|err| {
            I18nError::new(
                I18nErrorKind::SyntaxError,
                format!("invalid en langid: {err}"),
            )
        })?;
        let fa_bundle = build_bundle(fa_lang, fa_src, "fa")?;
        let en_bundle = build_bundle(en_lang, en_src, "en")?;

        Ok(Self {
            fa_bundle,
            en_bundle,
            fa_meta,
            en_meta,
        })
    }

    /// Looks up context metadata (`.surface`, `.situation`, variables) for `key` in `locale`.
    #[must_use]
    pub fn metadata(&self, locale: Locale, key: &str) -> Option<&MessageMetadata> {
        match locale {
            Locale::Fa => self.fa_meta.get(key),
            Locale::En => self.en_meta.get(key),
        }
    }

    /// Formats `key` strictly in `locale` with no cross-locale fallback.
    ///
    /// # Errors
    ///
    /// Returns [`I18nError`] if `key` is absent or formatting reports any Fluent error
    /// (for example missing placeable arguments).
    pub fn format(
        &self,
        locale: Locale,
        key: &str,
        args: &[(&str, &str)],
    ) -> Result<String, I18nError> {
        let bundle = match locale {
            Locale::Fa => &self.fa_bundle,
            Locale::En => &self.en_bundle,
        };
        let message = bundle.get_message(key).ok_or_else(|| {
            I18nError::new(
                I18nErrorKind::MissingMessage,
                format!("missing message `{key}` in locale `{}`", locale.as_tag()),
            )
        })?;
        let pattern = message.value().ok_or_else(|| {
            I18nError::new(
                I18nErrorKind::MissingMessage,
                format!("message `{key}` has no value pattern"),
            )
        })?;

        let mut fluent_args = FluentArgs::new();
        for (name, val) in args {
            fluent_args.set(*name, FluentValue::from(*val));
        }
        let args_ref = if args.is_empty() {
            None
        } else {
            Some(&fluent_args)
        };

        let mut errors = Vec::new();
        let formatted = bundle.format_pattern(pattern, args_ref, &mut errors);
        if !errors.is_empty() {
            return Err(I18nError::new(
                I18nErrorKind::FormattingFailed,
                format!("formatting `{key}` failed: {errors:?}"),
            ));
        }

        // Fluent wraps placeables in Unicode bidi isolation marks (U+2068 / U+2069); keep output
        // deterministic while preserving ZWNJ (U+200C) and localized content.
        Ok(formatted.replace(['\u{2068}', '\u{2069}'], ""))
    }
}

fn build_bundle(
    lang: unic_langid::LanguageIdentifier,
    src: &str,
    label: &str,
) -> Result<FluentBundle<FluentResource>, I18nError> {
    let resource = FluentResource::try_new(src.to_owned()).map_err(|(_, errs)| {
        I18nError::new(
            I18nErrorKind::SyntaxError,
            format!("{label} Fluent resource parse errors: {errs:?}"),
        )
    })?;
    let mut bundle = FluentBundle::new_concurrent(vec![lang]);
    bundle.set_use_isolating(false);
    bundle.add_resource(resource).map_err(|errs| {
        I18nError::new(
            I18nErrorKind::DuplicateMessage,
            format!("{label} bundle resource errors: {errs:?}"),
        )
    })?;
    Ok(bundle)
}

fn inspect_resource(
    locale_label: &str,
    src: &str,
) -> Result<BTreeMap<String, MessageMetadata>, I18nError> {
    let ast = fluent_syntax::parser::parse(src).map_err(|(_, errs)| {
        I18nError::new(
            I18nErrorKind::SyntaxError,
            format!("{locale_label} Fluent syntax error: {errs:?}"),
        )
    })?;

    let mut out = BTreeMap::new();
    for entry in &ast.body {
        if let Entry::Message(msg) = entry {
            let id = msg.id.name.to_owned();
            let pattern = msg.value.as_ref().ok_or_else(|| {
                I18nError::new(
                    I18nErrorKind::MissingMessage,
                    format!("{locale_label} message `{id}` has no value"),
                )
            })?;

            let mut variables = BTreeSet::new();
            let mut literal_text = String::new();
            collect_pattern_info(pattern, &mut variables, &mut literal_text);

            let mut surface: Option<String> = None;
            let mut situation: Option<String> = None;
            for attr in &msg.attributes {
                match attr.id.name {
                    "surface" => surface = Some(extract_static_text(&attr.value)?),
                    "situation" => situation = Some(extract_static_text(&attr.value)?),
                    _ => {}
                }
            }

            let surface = surface.ok_or_else(|| {
                I18nError::new(
                    I18nErrorKind::MissingContextMetadata,
                    format!("{locale_label} message `{id}` missing `.surface` attribute"),
                )
            })?;
            let situation = situation.ok_or_else(|| {
                I18nError::new(
                    I18nErrorKind::MissingContextMetadata,
                    format!("{locale_label} message `{id}` missing `.situation` attribute"),
                )
            })?;

            if !ALLOWED_SURFACES.contains(&surface.as_str()) {
                return Err(I18nError::new(
                    I18nErrorKind::MissingContextMetadata,
                    format!("{locale_label} message `{id}` has unknown surface `{surface}`"),
                ));
            }
            if !ALLOWED_SITUATIONS.contains(&situation.as_str()) {
                return Err(I18nError::new(
                    I18nErrorKind::MissingContextMetadata,
                    format!("{locale_label} message `{id}` has unknown situation `{situation}`"),
                ));
            }

            validate_copy_guardrails(locale_label, &id, &situation, &literal_text)?;

            if out
                .insert(
                    id.clone(),
                    MessageMetadata {
                        surface,
                        situation,
                        variables,
                    },
                )
                .is_some()
            {
                return Err(I18nError::new(
                    I18nErrorKind::DuplicateMessage,
                    format!("duplicate message `{id}` in `{locale_label}`"),
                ));
            }
        }
    }

    Ok(out)
}

fn extract_static_text(pattern: &Pattern<&str>) -> Result<String, I18nError> {
    let mut text = String::new();
    for elem in &pattern.elements {
        match elem {
            PatternElement::TextElement { value } => text.push_str(value),
            PatternElement::Placeable { .. } => {
                return Err(I18nError::new(
                    I18nErrorKind::MissingContextMetadata,
                    "context metadata attributes must be static text",
                ));
            }
        }
    }
    let trimmed = text.trim().to_owned();
    if trimmed.is_empty() {
        return Err(I18nError::new(
            I18nErrorKind::MissingContextMetadata,
            "context metadata attribute cannot be empty",
        ));
    }
    Ok(trimmed)
}

fn collect_pattern_info(
    pattern: &Pattern<&str>,
    variables: &mut BTreeSet<String>,
    literal_text: &mut String,
) {
    for element in &pattern.elements {
        match element {
            PatternElement::TextElement { value } => literal_text.push_str(value),
            PatternElement::Placeable { expression } => {
                collect_expression_vars(expression, variables, literal_text);
            }
        }
    }
}

fn collect_expression_vars(
    expr: &Expression<&str>,
    variables: &mut BTreeSet<String>,
    literal_text: &mut String,
) {
    match expr {
        Expression::Inline(inline) => collect_inline_vars(inline, variables, literal_text),
        Expression::Select { selector, variants } => {
            collect_inline_vars(selector, variables, literal_text);
            for variant in variants {
                collect_pattern_info(&variant.value, variables, literal_text);
            }
        }
    }
}

fn collect_inline_vars(
    inline: &InlineExpression<&str>,
    variables: &mut BTreeSet<String>,
    literal_text: &mut String,
) {
    match inline {
        InlineExpression::VariableReference { id } => {
            variables.insert(id.name.to_owned());
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            for pos in &arguments.positional {
                collect_inline_vars(pos, variables, literal_text);
            }
            for named in &arguments.named {
                collect_inline_vars(&named.value, variables, literal_text);
            }
        }
        InlineExpression::Placeable { expression } => {
            collect_expression_vars(expression, variables, literal_text);
        }
        InlineExpression::StringLiteral { value } => literal_text.push_str(value),
        _ => {}
    }
}

fn validate_copy_guardrails(
    locale_label: &str,
    id: &str,
    situation: &str,
    text: &str,
) -> Result<(), I18nError> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("taneh") || text.contains("تنه") || text.contains("تن خور") {
        return Err(I18nError::new(
            I18nErrorKind::ForbiddenBrandingOrEmoji,
            format!("{locale_label} message `{id}` violates Tankhor / تن‌خور naming rules"),
        ));
    }
    if matches!(situation, "validation" | "security" | "transport")
        && text.chars().any(is_emoji_like)
    {
        return Err(I18nError::new(
            I18nErrorKind::ForbiddenBrandingOrEmoji,
            format!("{locale_label} message `{id}` contains forbidden emoji in `{situation}` copy"),
        ));
    }
    Ok(())
}

const fn is_emoji_like(ch: char) -> bool {
    let code = ch as u32;
    (code >= 0x1F300 && code <= 0x1FAFF) || (code >= 0x2600 && code <= 0x27BF)
}
