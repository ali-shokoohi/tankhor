//! Canonical launch flag registry and typed flag values.

use kernel::{
    error::{DomainError, ErrorCode},
    ids::VersionId,
};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FlagKey {
    PartnerWidgetApi,
    PercentageRollout,
    PerUserUsageLimits,
    Ticketing,
    ExternalNotifications,
    PaidShareBilling,
}

impl FlagKey {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PartnerWidgetApi => "partner_widget_api",
            Self::PercentageRollout => "percentage_rollout",
            Self::PerUserUsageLimits => "per_user_usage_limits",
            Self::Ticketing => "ticketing",
            Self::ExternalNotifications => "external_notifications",
            Self::PaidShareBilling => "paid_share_billing",
        }
    }
}

impl FromStr for FlagKey {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        launch_flags()
            .into_iter()
            .map(|definition| definition.key)
            .find(|key| key.as_str() == value)
            .ok_or_else(|| DomainError::validation(ErrorCode::InvalidValue))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagDefinition {
    key: FlagKey,
    initial_value: bool,
    fa_label_key: &'static str,
    en_label_key: &'static str,
    description_key: &'static str,
}

impl FlagDefinition {
    #[must_use]
    pub const fn key(self) -> FlagKey {
        self.key
    }

    #[must_use]
    pub const fn initial_value(self) -> bool {
        self.initial_value
    }

    #[must_use]
    pub const fn fa_label_key(self) -> &'static str {
        self.fa_label_key
    }

    #[must_use]
    pub const fn en_label_key(self) -> &'static str {
        self.en_label_key
    }

    #[must_use]
    pub const fn description_key(self) -> &'static str {
        self.description_key
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag {
    subject: FlagKey,
    value: bool,
    version: VersionId,
}

impl Flag {
    #[must_use]
    pub const fn new(subject: FlagKey, value: bool, version: VersionId) -> Self {
        Self {
            subject,
            value,
            version,
        }
    }

    #[must_use]
    pub const fn subject(self) -> FlagKey {
        self.subject
    }

    #[must_use]
    pub const fn value(self) -> bool {
        self.value
    }

    #[must_use]
    pub const fn version(self) -> VersionId {
        self.version
    }
}

#[must_use]
pub fn launch_flags() -> Vec<FlagDefinition> {
    [
        FlagKey::PartnerWidgetApi,
        FlagKey::PercentageRollout,
        FlagKey::PerUserUsageLimits,
        FlagKey::Ticketing,
        FlagKey::ExternalNotifications,
        FlagKey::PaidShareBilling,
    ]
    .into_iter()
    .map(|key| FlagDefinition {
        key,
        initial_value: false,
        fa_label_key: label_key(key),
        en_label_key: label_key(key),
        description_key: description_key(key),
    })
    .collect()
}

const fn label_key(key: FlagKey) -> &'static str {
    match key {
        FlagKey::PartnerWidgetApi => "flag-partner_widget_api-label",
        FlagKey::PercentageRollout => "flag-percentage_rollout-label",
        FlagKey::PerUserUsageLimits => "flag-per_user_usage_limits-label",
        FlagKey::Ticketing => "flag-ticketing-label",
        FlagKey::ExternalNotifications => "flag-external_notifications-label",
        FlagKey::PaidShareBilling => "flag-paid_share_billing-label",
    }
}

const fn description_key(key: FlagKey) -> &'static str {
    match key {
        FlagKey::PartnerWidgetApi => "flag-partner_widget_api-description",
        FlagKey::PercentageRollout => "flag-percentage_rollout-description",
        FlagKey::PerUserUsageLimits => "flag-per_user_usage_limits-description",
        FlagKey::Ticketing => "flag-ticketing-description",
        FlagKey::ExternalNotifications => "flag-external_notifications-description",
        FlagKey::PaidShareBilling => "flag-paid_share_billing-description",
    }
}

#[cfg(test)]
mod tests {
    use super::{Flag, FlagKey, launch_flags};
    use kernel::ids::VersionId;
    use std::str::FromStr;

    #[test]
    fn six_launch_flags_start_off() {
        const KEYS: [&str; 6] = [
            "partner_widget_api",
            "percentage_rollout",
            "per_user_usage_limits",
            "ticketing",
            "external_notifications",
            "paid_share_billing",
        ];
        let definitions = launch_flags();
        assert_eq!(definitions.len(), KEYS.len());
        for key in KEYS {
            let definition = definitions
                .iter()
                .find(|definition| definition.key().as_str() == key)
                .unwrap();
            assert_eq!(FlagKey::from_str(key).unwrap(), definition.key());
            assert!(!definition.initial_value());
            assert_eq!(definition.fa_label_key(), format!("flag-{key}-label"));
            assert_eq!(definition.en_label_key(), format!("flag-{key}-label"));
            assert_eq!(
                definition.description_key(),
                format!("flag-{key}-description")
            );
        }

        let version = VersionId::from_str("018f2f7c-8a3c-7def-9abc-1234567890ab").unwrap();
        let flag = Flag::new(FlagKey::Ticketing, false, version);
        assert_eq!(flag.subject(), FlagKey::Ticketing);
        assert!(!flag.value());
        assert_eq!(flag.version(), version);
    }
}
