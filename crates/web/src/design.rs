//! Authoritative design token parser and WCAG 2.2 AA unrounded sRGB contrast validator.

use std::collections::{BTreeMap, BTreeSet};

const AUTHORITATIVE_TOML: &str = include_str!("../design/tokens.toml");

const AUTHORITATIVE_ROLES: [&str; 16] = [
    "background",
    "surface",
    "surfaceMuted",
    "textPrimary",
    "textSecondary",
    "brandVelvet",
    "curtainBase",
    "sky",
    "action",
    "onAction",
    "border",
    "borderControl",
    "onCurtain",
    "success",
    "warning",
    "danger",
];

/// Theme mode for contrast evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
}

/// WCAG 2.2 text size classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSizeRole {
    /// Normal text requiring unrounded contrast >= 4.5:1.
    Normal,
    /// Large text requiring unrounded contrast >= 3.0:1.
    Large,
}

impl TextSizeRole {
    #[must_use]
    pub const fn min_contrast_ratio(self) -> f64 {
        match self {
            Self::Normal => 4.5,
            Self::Large => 3.0,
        }
    }
}

/// Failure kind returned by [`DesignTokens::validate_all`] or parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContrastErrorKind {
    InvalidTokenFile,
    InvalidColorFormat,
    UnknownTokenRole,
    DecorativeTokenUsedAsControlBoundary,
    ProhibitedPairUsed,
    TextContrastTooLow,
    ControlContrastTooLow,
}

/// Typed contrast/token validation error.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ContrastError {
    kind: ContrastErrorKind,
    message: String,
}

impl ContrastError {
    #[must_use]
    pub fn new(kind: ContrastErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ContrastErrorKind {
        self.kind
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TextPairRule {
    name: String,
    foreground: String,
    background: String,
    size_role: TextSizeRole,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ControlBoundaryRule {
    name: String,
    token: String,
    against: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProhibitedPairRule {
    name: String,
    theme: ThemeMode,
    foreground: String,
    background: String,
    size_role: TextSizeRole,
}

/// Parsed design tokens from `crates/web/design/tokens.toml`.
#[derive(Clone, Debug)]
pub struct DesignTokens {
    brand_fa: String,
    brand_en: String,
    font_stack: Vec<String>,
    light_colors: BTreeMap<String, String>,
    dark_colors: BTreeMap<String, String>,
    decorative_only_tokens: BTreeSet<String>,
    text_pairs: Vec<TextPairRule>,
    control_boundaries: Vec<ControlBoundaryRule>,
    prohibited_pairs: Vec<ProhibitedPairRule>,
}

impl DesignTokens {
    /// Load and parse the canonical embedded `crates/web/design/tokens.toml`.
    ///
    /// # Errors
    ///
    /// Returns [`ContrastError`] if `tokens.toml` is malformed or missing required sections.
    pub fn load_authoritative() -> Result<Self, ContrastError> {
        Self::parse_toml(AUTHORITATIVE_TOML)
    }

    /// Parse a token TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`ContrastError`] if the TOML structure or values are invalid.
    pub fn parse_toml(raw: &str) -> Result<Self, ContrastError> {
        let (sections, array_tables) = parse_raw_toml_tables(raw);

        let meta = sections.get("meta").ok_or_else(|| {
            ContrastError::new(
                ContrastErrorKind::InvalidTokenFile,
                "missing [meta] section",
            )
        })?;
        let brand_fa = unquote(meta.get("brand_fa").ok_or_else(|| {
            ContrastError::new(ContrastErrorKind::InvalidTokenFile, "missing meta.brand_fa")
        })?)?;
        let brand_en = unquote(meta.get("brand_en").ok_or_else(|| {
            ContrastError::new(ContrastErrorKind::InvalidTokenFile, "missing meta.brand_en")
        })?)?;

        let typography = sections.get("typography").ok_or_else(|| {
            ContrastError::new(
                ContrastErrorKind::InvalidTokenFile,
                "missing [typography] section",
            )
        })?;
        let font_stack = parse_string_list(typography.get("font_stack").ok_or_else(|| {
            ContrastError::new(
                ContrastErrorKind::InvalidTokenFile,
                "missing typography.font_stack",
            )
        })?)?;

        let light_colors = parse_color_map(sections.get("colors.light").ok_or_else(|| {
            ContrastError::new(
                ContrastErrorKind::InvalidTokenFile,
                "missing [colors.light] section",
            )
        })?)?;
        let dark_colors = parse_color_map(sections.get("colors.dark").ok_or_else(|| {
            ContrastError::new(
                ContrastErrorKind::InvalidTokenFile,
                "missing [colors.dark] section",
            )
        })?)?;

        let contrast_sec = sections.get("contrast").ok_or_else(|| {
            ContrastError::new(
                ContrastErrorKind::InvalidTokenFile,
                "missing [contrast] section",
            )
        })?;
        let decorative_only_tokens =
            parse_string_list(contrast_sec.get("decorative_only_tokens").ok_or_else(|| {
                ContrastError::new(
                    ContrastErrorKind::InvalidTokenFile,
                    "missing contrast.decorative_only_tokens",
                )
            })?)?
            .into_iter()
            .collect::<BTreeSet<_>>();

        let (text_pairs, control_boundaries, prohibited_pairs) =
            parse_contrast_array_rules(&array_tables)?;

        Ok(Self {
            brand_fa,
            brand_en,
            font_stack,
            light_colors,
            dark_colors,
            decorative_only_tokens,
            text_pairs,
            control_boundaries,
            prohibited_pairs,
        })
    }

    #[must_use]
    pub fn brand_fa(&self) -> &str {
        &self.brand_fa
    }

    #[must_use]
    pub fn brand_en(&self) -> &str {
        &self.brand_en
    }

    #[must_use]
    pub fn font_stack(&self) -> &[String] {
        &self.font_stack
    }

    /// Override a theme colour value (used by negative tests to verify contrast failures).
    ///
    /// # Errors
    ///
    /// Returns [`ContrastError`] if the role is unknown or the hex value is malformed.
    pub fn set_theme_color(
        &mut self,
        theme: ThemeMode,
        role: &str,
        hex: &str,
    ) -> Result<(), ContrastError> {
        let _ = parse_hex_rgb(hex)?;
        let map = match theme {
            ThemeMode::Light => &mut self.light_colors,
            ThemeMode::Dark => &mut self.dark_colors,
        };
        if !map.contains_key(role) {
            return Err(ContrastError::new(
                ContrastErrorKind::UnknownTokenRole,
                format!("unknown colour role `{role}`"),
            ));
        }
        map.insert(role.to_string(), hex.to_string());
        Ok(())
    }

    /// Add a text pair rule to validate.
    pub fn add_text_pair(
        &mut self,
        name: impl Into<String>,
        foreground: impl Into<String>,
        background: impl Into<String>,
        size_role: TextSizeRole,
    ) {
        self.text_pairs.push(TextPairRule {
            name: name.into(),
            foreground: foreground.into(),
            background: background.into(),
            size_role,
        });
    }

    /// Add a control boundary rule to validate.
    pub fn add_control_boundary(
        &mut self,
        name: impl Into<String>,
        token: impl Into<String>,
        against: impl Into<String>,
    ) {
        self.control_boundaries.push(ControlBoundaryRule {
            name: name.into(),
            token: token.into(),
            against: against.into(),
        });
    }

    /// Validate semantic role completeness, prohibited role pairings, decorative-border restrictions,
    /// and unrounded WCAG 2.2 AA contrast across both light and dark themes.
    ///
    /// # Errors
    ///
    /// Returns [`ContrastError`] on the first violated rule.
    pub fn validate_all(&self) -> Result<(), ContrastError> {
        validate_exact_roles(&self.light_colors, "light")?;
        validate_exact_roles(&self.dark_colors, "dark")?;

        // Check prohibited pairs first.
        for tp in &self.text_pairs {
            for prohib in &self.prohibited_pairs {
                if tp.foreground == prohib.foreground
                    && tp.background == prohib.background
                    && tp.size_role == prohib.size_role
                {
                    return Err(ContrastError::new(
                        ContrastErrorKind::ProhibitedPairUsed,
                        format!(
                            "text pair `{}` violates prohibited rule `{}` ({:?}: {} on {})",
                            tp.name, prohib.name, prohib.theme, tp.foreground, tp.background
                        ),
                    ));
                }
            }
        }

        // Check control boundary rules (including decorative token prohibition and >= 3.0:1 unrounded).
        for cb in &self.control_boundaries {
            if self.decorative_only_tokens.contains(&cb.token) {
                return Err(ContrastError::new(
                    ContrastErrorKind::DecorativeTokenUsedAsControlBoundary,
                    format!(
                        "control boundary `{}` uses decorative-only token `{}`",
                        cb.name, cb.token
                    ),
                ));
            }
            for (theme, colors) in [
                (ThemeMode::Light, &self.light_colors),
                (ThemeMode::Dark, &self.dark_colors),
            ] {
                let fg = lookup_color(colors, &cb.token)?;
                let bg = lookup_color(colors, &cb.against)?;
                let ratio = contrast_ratio_hex(fg, bg)?;
                if ratio < 3.0 {
                    return Err(ContrastError::new(
                        ContrastErrorKind::ControlContrastTooLow,
                        format!(
                            "control boundary `{}` ({theme:?}: {} on {}) has unrounded contrast {ratio:.6} < 3.0",
                            cb.name, cb.token, cb.against
                        ),
                    ));
                }
            }
        }

        // Check all text pairs in both themes using exact unrounded WCAG 2.2 ratios.
        for tp in &self.text_pairs {
            let min_ratio = tp.size_role.min_contrast_ratio();
            for (theme, colors) in [
                (ThemeMode::Light, &self.light_colors),
                (ThemeMode::Dark, &self.dark_colors),
            ] {
                let fg = lookup_color(colors, &tp.foreground)?;
                let bg = lookup_color(colors, &tp.background)?;
                let ratio = contrast_ratio_hex(fg, bg)?;
                if ratio < min_ratio {
                    return Err(ContrastError::new(
                        ContrastErrorKind::TextContrastTooLow,
                        format!(
                            "text pair `{}` ({theme:?}: {} on {}) has unrounded contrast {ratio:.6} < {min_ratio:.1}",
                            tp.name, tp.foreground, tp.background
                        ),
                    ));
                }
            }
        }

        // Also assert that every declared prohibited pair is indeed below threshold in its theme.
        for prohib in &self.prohibited_pairs {
            let colors = match prohib.theme {
                ThemeMode::Light => &self.light_colors,
                ThemeMode::Dark => &self.dark_colors,
            };
            let fg = lookup_color(colors, &prohib.foreground)?;
            let bg = lookup_color(colors, &prohib.background)?;
            let ratio = contrast_ratio_hex(fg, bg)?;
            if ratio >= prohib.size_role.min_contrast_ratio() {
                return Err(ContrastError::new(
                    ContrastErrorKind::InvalidTokenFile,
                    format!(
                        "prohibited pair `{}` unexpectedly meets contrast threshold ({ratio:.4})",
                        prohib.name
                    ),
                ));
            }
        }

        Ok(())
    }
}

/// Compute exact unrounded WCAG 2.2 relative-luminance contrast ratio between two `#RRGGBB` colours.
///
/// # Errors
///
/// Returns [`ContrastError`] if either string is not `#RRGGBB`.
pub fn contrast_ratio_hex(fg_hex: &str, bg_hex: &str) -> Result<f64, ContrastError> {
    let fg = parse_hex_rgb(fg_hex)?;
    let bg = parse_hex_rgb(bg_hex)?;
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    Ok((lighter + 0.05) / (darker + 0.05))
}

fn relative_luminance((r, g, b): (u8, u8, u8)) -> f64 {
    0.2126 * srgb_channel_linear(r)
        + 0.7152 * srgb_channel_linear(g)
        + 0.0722 * srgb_channel_linear(b)
}

fn srgb_channel_linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn parse_hex_rgb(hex: &str) -> Result<(u8, u8, u8), ContrastError> {
    if hex.len() != 7 || !hex.starts_with('#') {
        return Err(ContrastError::new(
            ContrastErrorKind::InvalidColorFormat,
            format!("expected #RRGGBB colour, got `{hex}`"),
        ));
    }
    let r = u8::from_str_radix(&hex[1..3], 16).map_err(|_| {
        ContrastError::new(
            ContrastErrorKind::InvalidColorFormat,
            format!("invalid red channel in `{hex}`"),
        )
    })?;
    let g = u8::from_str_radix(&hex[3..5], 16).map_err(|_| {
        ContrastError::new(
            ContrastErrorKind::InvalidColorFormat,
            format!("invalid green channel in `{hex}`"),
        )
    })?;
    let b = u8::from_str_radix(&hex[5..7], 16).map_err(|_| {
        ContrastError::new(
            ContrastErrorKind::InvalidColorFormat,
            format!("invalid blue channel in `{hex}`"),
        )
    })?;
    Ok((r, g, b))
}

type RawSections = BTreeMap<String, BTreeMap<String, String>>;
type RawArrayTables = BTreeMap<String, Vec<BTreeMap<String, String>>>;

fn parse_raw_toml_tables(raw: &str) -> (RawSections, RawArrayTables) {
    let mut sections: RawSections = BTreeMap::new();
    let mut array_tables: RawArrayTables = BTreeMap::new();
    let mut current_table: Option<String> = None;
    let mut current_array_table: Option<String> = None;

    for raw_line in raw.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("[[") && line.ends_with("]]") {
            let name = line[2..line.len() - 2].trim().to_string();
            array_tables
                .entry(name.clone())
                .or_default()
                .push(BTreeMap::new());
            current_array_table = Some(name);
            current_table = None;
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = line[1..line.len() - 1].trim().to_string();
            sections.entry(name.clone()).or_default();
            current_table = Some(name);
            current_array_table = None;
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            if let Some(arr_name) = &current_array_table {
                if let Some(last) = array_tables.get_mut(arr_name).and_then(|v| v.last_mut()) {
                    last.insert(key, val);
                }
            } else if let Some(sec_name) = &current_table {
                sections
                    .entry(sec_name.clone())
                    .or_default()
                    .insert(key, val);
            }
        }
    }
    (sections, array_tables)
}

type ParsedContrastRules = (
    Vec<TextPairRule>,
    Vec<ControlBoundaryRule>,
    Vec<ProhibitedPairRule>,
);

fn parse_contrast_array_rules(
    array_tables: &RawArrayTables,
) -> Result<ParsedContrastRules, ContrastError> {
    let mut text_pairs = Vec::new();
    for entry in array_tables
        .get("contrast.text_pairs")
        .cloned()
        .unwrap_or_default()
    {
        text_pairs.push(TextPairRule {
            name: unquote_field(&entry, "name")?,
            foreground: unquote_field(&entry, "foreground")?,
            background: unquote_field(&entry, "background")?,
            size_role: parse_size_role(&unquote_field(&entry, "size_role")?)?,
        });
    }

    let mut control_boundaries = Vec::new();
    for entry in array_tables
        .get("contrast.control_boundaries")
        .cloned()
        .unwrap_or_default()
    {
        control_boundaries.push(ControlBoundaryRule {
            name: unquote_field(&entry, "name")?,
            token: unquote_field(&entry, "token")?,
            against: unquote_field(&entry, "against")?,
        });
    }

    let mut prohibited_pairs = Vec::new();
    for entry in array_tables
        .get("contrast.prohibited_pairs")
        .cloned()
        .unwrap_or_default()
    {
        let theme = match unquote_field(&entry, "theme")?.as_str() {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            other => {
                return Err(ContrastError::new(
                    ContrastErrorKind::InvalidTokenFile,
                    format!("unknown theme `{other}`"),
                ));
            }
        };
        prohibited_pairs.push(ProhibitedPairRule {
            name: unquote_field(&entry, "name")?,
            theme,
            foreground: unquote_field(&entry, "foreground")?,
            background: unquote_field(&entry, "background")?,
            size_role: parse_size_role(&unquote_field(&entry, "size_role")?)?,
        });
    }
    Ok((text_pairs, control_boundaries, prohibited_pairs))
}

fn validate_exact_roles(
    map: &BTreeMap<String, String>,
    theme_name: &str,
) -> Result<(), ContrastError> {
    let expected: BTreeSet<&str> = AUTHORITATIVE_ROLES.into_iter().collect();
    let actual: BTreeSet<&str> = map.keys().map(String::as_str).collect();
    if actual != expected {
        return Err(ContrastError::new(
            ContrastErrorKind::InvalidTokenFile,
            format!("theme `{theme_name}` colour roles do not match §12.2 authoritative set"),
        ));
    }
    Ok(())
}

fn lookup_color<'a>(
    colors: &'a BTreeMap<String, String>,
    role: &str,
) -> Result<&'a str, ContrastError> {
    colors.get(role).map(String::as_str).ok_or_else(|| {
        ContrastError::new(
            ContrastErrorKind::UnknownTokenRole,
            format!("unknown colour role `{role}`"),
        )
    })
}

fn parse_color_map(
    raw_map: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, ContrastError> {
    let mut parsed = BTreeMap::new();
    for (k, v) in raw_map {
        let hex = unquote(v)?;
        let _ = parse_hex_rgb(&hex)?;
        parsed.insert(k.clone(), hex);
    }
    Ok(parsed)
}

fn parse_size_role(raw: &str) -> Result<TextSizeRole, ContrastError> {
    match raw {
        "normal" => Ok(TextSizeRole::Normal),
        "large" => Ok(TextSizeRole::Large),
        other => Err(ContrastError::new(
            ContrastErrorKind::InvalidTokenFile,
            format!("unknown size_role `{other}`"),
        )),
    }
}

fn unquote_field(map: &BTreeMap<String, String>, field: &str) -> Result<String, ContrastError> {
    let raw = map.get(field).ok_or_else(|| {
        ContrastError::new(
            ContrastErrorKind::InvalidTokenFile,
            format!("missing field `{field}`"),
        )
    })?;
    unquote(raw)
}

fn unquote(raw: &str) -> Result<String, ContrastError> {
    let trimmed = raw.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return Err(ContrastError::new(
            ContrastErrorKind::InvalidTokenFile,
            format!("expected quoted string, got `{trimmed}`"),
        ));
    }
    Ok(trimmed[1..trimmed.len() - 1].to_string())
}

fn parse_string_list(raw: &str) -> Result<Vec<String>, ContrastError> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(ContrastError::new(
            ContrastErrorKind::InvalidTokenFile,
            format!("expected TOML string array, got `{trimmed}`"),
        ));
    }
    trimmed[1..trimmed.len() - 1]
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(unquote)
        .collect()
}

fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    for (idx, ch) in line.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == '#' && !in_quotes {
            return &line[..idx];
        }
    }
    line
}
