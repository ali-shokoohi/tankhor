//! Typed settings registry and checked value policies.
//! FR-006/007, FR-CON-32, AC-ADM-008/009/037.

use super::formats::FormatSet;
use kernel::{
    error::{DomainError, ErrorCode},
    ids::VersionId,
};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SettingKey {
    BuyerTempPhotoDays,
    StudioTempUploadDays,
    StudioAutoDraftDays,
    AccountRecoveryDays,
    StudioMaxCards,
    BuyerMaxPermanentPhotos,
    UploadMaxMb,
    UploadMinShortestEdgePx,
    GenerationMaxAttempts,
    UploadFormats,
}

impl SettingKey {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BuyerTempPhotoDays => "buyer_temp_photo_days",
            Self::StudioTempUploadDays => "studio_temp_upload_days",
            Self::StudioAutoDraftDays => "studio_auto_draft_days",
            Self::AccountRecoveryDays => "account_recovery_days",
            Self::StudioMaxCards => "studio_max_cards",
            Self::BuyerMaxPermanentPhotos => "buyer_max_permanent_photos",
            Self::UploadMaxMb => "upload_max_mb",
            Self::UploadMinShortestEdgePx => "upload_min_shortest_edge_px",
            Self::GenerationMaxAttempts => "generation_max_attempts",
            Self::UploadFormats => "upload_formats",
        }
    }
}

impl fmt::Display for SettingKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SettingKey {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        initial_numeric_settings()
            .into_iter()
            .map(|setting| setting.key)
            .chain([SettingKey::UploadFormats])
            .find(|key| key.as_str() == value)
            .ok_or_else(invalid_setting)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingUnit {
    DaysFromOriginalUpload,
    DaysFromLastActivity,
    DaysFromDeletionRequest,
    Cards,
    Photos,
    Megabytes,
    Pixels,
    TotalAttempts,
    FormatMask,
}

impl SettingUnit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DaysFromOriginalUpload => "days_from_original_upload",
            Self::DaysFromLastActivity => "days_from_last_activity",
            Self::DaysFromDeletionRequest => "days_from_deletion_request",
            Self::Cards => "cards",
            Self::Photos => "photos",
            Self::Megabytes => "megabytes",
            Self::Pixels => "pixels",
            Self::TotalAttempts => "total_attempts",
            Self::FormatMask => "format_mask",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericPolicy {
    minimum: u32,
    maximum: u32,
}

impl NumericPolicy {
    #[must_use]
    pub const fn new(minimum: u32, maximum: u32) -> Self {
        Self { minimum, maximum }
    }

    #[must_use]
    pub const fn minimum(self) -> u32 {
        self.minimum
    }

    #[must_use]
    pub const fn maximum(self) -> u32 {
        self.maximum
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingValue {
    Number(i128),
    Boolean(bool),
    Formats(FormatSet),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting<T> {
    key: SettingKey,
    value: T,
    policy: NumericPolicy,
    unit: SettingUnit,
    fa_label_key: &'static str,
    en_label_key: &'static str,
    live_version: Option<VersionId>,
    history: Vec<VersionId>,
}

impl<T: Copy> Setting<T> {
    #[must_use]
    pub const fn value(&self) -> T {
        self.value
    }

    #[must_use]
    pub const fn policy(&self) -> NumericPolicy {
        self.policy
    }

    #[must_use]
    pub const fn key(&self) -> SettingKey {
        self.key
    }

    #[must_use]
    pub const fn unit(&self) -> SettingUnit {
        self.unit
    }

    #[must_use]
    pub const fn fa_label_key(&self) -> &'static str {
        self.fa_label_key
    }

    #[must_use]
    pub const fn en_label_key(&self) -> &'static str {
        self.en_label_key
    }

    #[must_use]
    pub const fn live_version(&self) -> Option<VersionId> {
        self.live_version
    }

    #[must_use]
    pub fn history(&self) -> &[VersionId] {
        &self.history
    }
}

impl Setting<u32> {
    /// # Errors
    /// Rejects non-numeric or out-of-range values.
    #[allow(clippy::needless_pass_by_value)]
    pub fn validate(&self, value: SettingValue) -> Result<u32, DomainError> {
        let SettingValue::Number(value) = value else {
            return Err(invalid_setting());
        };
        if value < i128::from(self.policy.minimum)
            || value > i128::from(self.policy.maximum)
            || value > i128::from(u32::MAX)
        {
            return Err(invalid_setting());
        }
        u32::try_from(value).map_err(|_| invalid_setting())
    }

    /// # Errors
    /// Rejects non-numeric strings or out-of-range values.
    pub fn parse_value(&self, value: &str) -> Result<u32, DomainError> {
        let value = value.parse::<i128>().map_err(|_| invalid_setting())?;
        self.validate(SettingValue::Number(value))
    }
}

impl Setting<FormatSet> {
    /// # Errors
    /// Rejects non-format values or invalid format masks.
    #[allow(clippy::needless_pass_by_value)]
    pub fn validate(&self, value: SettingValue) -> Result<FormatSet, DomainError> {
        let SettingValue::Formats(value) = value else {
            return Err(invalid_setting());
        };
        FormatSet::from_mask(value.mask())
    }

    /// # Errors
    /// Rejects unparseable format sets.
    pub fn parse_value(&self, value: &str) -> Result<FormatSet, DomainError> {
        value.parse().map_err(|_| invalid_setting())
    }
}

#[must_use]
pub fn initial_numeric_settings() -> Vec<Setting<u32>> {
    use SettingKey as Key;
    use SettingUnit as Unit;
    [
        (
            Key::BuyerTempPhotoDays,
            14,
            1,
            30,
            Unit::DaysFromOriginalUpload,
        ),
        (
            Key::StudioTempUploadDays,
            30,
            7,
            90,
            Unit::DaysFromOriginalUpload,
        ),
        (
            Key::StudioAutoDraftDays,
            30,
            1,
            90,
            Unit::DaysFromLastActivity,
        ),
        (
            Key::AccountRecoveryDays,
            30,
            14,
            90,
            Unit::DaysFromDeletionRequest,
        ),
        (Key::StudioMaxCards, 50, 1, 100, Unit::Cards),
        (Key::BuyerMaxPermanentPhotos, 5, 1, 10, Unit::Photos),
        (Key::UploadMaxMb, 15, 1, 25, Unit::Megabytes),
        (Key::UploadMinShortestEdgePx, 512, 256, 1024, Unit::Pixels),
        (Key::GenerationMaxAttempts, 3, 1, 3, Unit::TotalAttempts),
    ]
    .into_iter()
    .map(|(key, value, minimum, maximum, unit)| Setting {
        key,
        value,
        policy: NumericPolicy::new(minimum, maximum),
        unit,
        fa_label_key: label_key(key),
        en_label_key: label_key(key),
        live_version: None,
        history: Vec::new(),
    })
    .collect()
}

#[must_use]
pub fn initial_upload_formats() -> Setting<FormatSet> {
    Setting {
        key: SettingKey::UploadFormats,
        value: FormatSet::all(),
        policy: NumericPolicy::new(1, 15),
        unit: SettingUnit::FormatMask,
        fa_label_key: label_key(SettingKey::UploadFormats),
        en_label_key: label_key(SettingKey::UploadFormats),
        live_version: None,
        history: Vec::new(),
    }
}

const fn label_key(key: SettingKey) -> &'static str {
    match key {
        SettingKey::BuyerTempPhotoDays => "setting-buyer_temp_photo_days-label",
        SettingKey::StudioTempUploadDays => "setting-studio_temp_upload_days-label",
        SettingKey::StudioAutoDraftDays => "setting-studio_auto_draft_days-label",
        SettingKey::AccountRecoveryDays => "setting-account_recovery_days-label",
        SettingKey::StudioMaxCards => "setting-studio_max_cards-label",
        SettingKey::BuyerMaxPermanentPhotos => "setting-buyer_max_permanent_photos-label",
        SettingKey::UploadMaxMb => "setting-upload_max_mb-label",
        SettingKey::UploadMinShortestEdgePx => "setting-upload_min_shortest_edge_px-label",
        SettingKey::GenerationMaxAttempts => "setting-generation_max_attempts-label",
        SettingKey::UploadFormats => "setting-upload_formats-label",
    }
}

fn invalid_setting() -> DomainError {
    DomainError::validation(ErrorCode::InvalidValue)
}

#[cfg(test)]
mod tests {
    use super::{SettingKey, SettingValue, initial_numeric_settings};
    use proptest::prelude::*;
    use std::str::FromStr;

    const NUMERIC_TRIPLES: [(&str, u32, u32, u32); 9] = [
        ("buyer_temp_photo_days", 14, 1, 30),
        ("studio_temp_upload_days", 30, 7, 90),
        ("studio_auto_draft_days", 30, 1, 90),
        ("account_recovery_days", 30, 14, 90),
        ("studio_max_cards", 50, 1, 100),
        ("buyer_max_permanent_photos", 5, 1, 10),
        ("upload_max_mb", 15, 1, 25),
        ("upload_min_shortest_edge_px", 512, 256, 1024),
        ("generation_max_attempts", 3, 1, 3),
    ];

    #[test]
    fn initial_settings_have_decided_values_and_complete_metadata() {
        let settings = initial_numeric_settings();
        assert_eq!(settings.len(), NUMERIC_TRIPLES.len());
        for (key, default, minimum, maximum) in NUMERIC_TRIPLES {
            let setting = settings
                .iter()
                .find(|setting| setting.key().as_str() == key)
                .unwrap();
            assert_eq!(SettingKey::from_str(key).unwrap(), setting.key());
            assert_eq!(setting.value(), default);
            assert_eq!(setting.policy().minimum(), minimum);
            assert_eq!(setting.policy().maximum(), maximum);
            assert_eq!(setting.fa_label_key(), format!("setting-{key}-label"));
            assert_eq!(setting.en_label_key(), format!("setting-{key}-label"));
            assert!(!setting.unit().as_str().is_empty());
            assert_eq!(setting.live_version(), None);
            assert!(setting.history().is_empty());
        }
        let mut identities = settings
            .iter()
            .map(|setting| setting.key().as_str())
            .collect::<Vec<_>>();
        identities.sort_unstable();
        identities.dedup();
        assert_eq!(identities.len(), NUMERIC_TRIPLES.len());
    }

    #[test]
    fn settings_accept_inclusive_bounds_without_changing_defaults() {
        for setting in initial_numeric_settings() {
            let original = setting.value();
            for boundary in [setting.policy().minimum(), setting.policy().maximum()] {
                assert_eq!(
                    setting
                        .validate(SettingValue::Number(i128::from(boundary)))
                        .unwrap(),
                    boundary
                );
                assert_eq!(setting.value(), original);
            }
        }
    }

    proptest! {
        #[test]
        fn setting_rejects_values_outside_declared_policy(candidate in -2048i128..2048) {
            for setting in initial_numeric_settings() {
                let result = setting.validate(SettingValue::Number(candidate));
                let in_bounds = candidate >= i128::from(setting.policy().minimum())
                    && candidate <= i128::from(setting.policy().maximum());
                prop_assert_eq!(result.is_ok(), in_bounds);
            }
        }
    }

    #[test]
    fn setting_rejects_wrong_kind_fraction_and_overflow() {
        for setting in initial_numeric_settings() {
            assert!(setting.validate(SettingValue::Boolean(true)).is_err());
            assert!(
                setting
                    .validate(SettingValue::Number(i128::from(u32::MAX) + 1))
                    .is_err()
            );
            assert!(setting.validate(SettingValue::Number(i128::MIN)).is_err());
            assert!(setting.parse_value("1.5").is_err());
        }
    }
}
