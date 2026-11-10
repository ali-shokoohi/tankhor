//! Bilingual locale, derived layout direction and console theme values.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error returned when parsing an unsupported presentation preference.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PresentationParseError {
    /// Locale tag is neither `fa` nor `en`.
    #[error("unsupported locale")]
    UnsupportedLocale,
    /// Theme identifier is not `light`, `dark` or `system`.
    #[error("unsupported theme")]
    UnsupportedTheme,
}

/// Supported console and product locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    /// Persian (`fa`), default locale, rendered right-to-left.
    #[default]
    Fa,
    /// English (`en`), rendered left-to-right.
    En,
}

impl Locale {
    /// Canonical BCP 47 language subtag (`fa` or `en`).
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::Fa => "fa",
            Self::En => "en",
        }
    }

    /// Derive layout direction from the locale. Direction is never independently editable.
    #[must_use]
    pub const fn direction(self) -> Direction {
        match self {
            Self::Fa => Direction::Rtl,
            Self::En => Direction::Ltr,
        }
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_tag())
    }
}

impl FromStr for Locale {
    type Err = PresentationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "fa" => Ok(Self::Fa),
            "en" => Ok(Self::En),
            _ => Err(PresentationParseError::UnsupportedLocale),
        }
    }
}

/// Document writing direction derived exclusively from [`Locale`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Right-to-left (`rtl`).
    Rtl,
    /// Left-to-right (`ltr`).
    Ltr,
}

impl Direction {
    /// HTML `dir` attribute value (`rtl` or `ltr`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rtl => "rtl",
            Self::Ltr => "ltr",
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Console color-scheme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Explicit light palette.
    Light,
    /// Explicit dark palette.
    Dark,
    /// Follow device/browser color-scheme preference.
    #[default]
    System,
}

impl Theme {
    /// Canonical string value (`light`, `dark` or `system`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }
}

impl fmt::Display for Theme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Theme {
    type Err = PresentationParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            "system" => Ok(Self::System),
            _ => Err(PresentationParseError::UnsupportedTheme),
        }
    }
}
