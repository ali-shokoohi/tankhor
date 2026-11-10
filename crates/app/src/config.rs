//! Validated application environment and coherent adapter-mode configuration.

use std::collections::BTreeMap;

/// Operating environment (`TANKHOR_ENV`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEnvironment {
    Development,
    Staging,
    Production,
}

/// Application-wide adapter selection (`TANKHOR_ADAPTERS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterMode {
    Real,
    Fake,
}

/// Sanitized configuration validation errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required TANKHOR_ENV variable")]
    MissingEnvironment,
    #[error("invalid TANKHOR_ENV value; expected development, staging, or production")]
    InvalidEnvironment,
    #[error("invalid TANKHOR_ADAPTERS value; expected real or fake")]
    InvalidAdapterMode,
    #[error("per-group adapter override `{key}` is forbidden; use one TANKHOR_ADAPTERS mode")]
    PerGroupAdapterOverride { key: String },
    #[error("TANKHOR_ADAPTERS=fake is forbidden when TANKHOR_ENV=production")]
    ProductionRejectsFakeAdapters,
}

/// Validated application-wide mode and environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    environment: RuntimeEnvironment,
    adapter_mode: AdapterMode,
}

impl AppConfig {
    /// Parses and validates configuration from process environment variables.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] when `TANKHOR_ENV` or `TANKHOR_ADAPTERS` is missing or
    /// invalid, a per-group adapter override variable is present, or `fake` mode is
    /// requested in `production`.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_pairs(std::env::vars())
    }

    /// Parses and validates configuration from `(key, value)` pairs.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] when `TANKHOR_ENV` or `TANKHOR_ADAPTERS` is missing or
    /// invalid, a per-group adapter override variable is present, or `fake` mode is
    /// requested in `production`.
    pub fn from_pairs<I, K, V>(pairs: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut map = BTreeMap::new();
        for (raw_key, raw_val) in pairs {
            let key = raw_key.as_ref();
            if is_per_group_adapter_key(key) {
                return Err(ConfigError::PerGroupAdapterOverride {
                    key: key.to_owned(),
                });
            }
            map.insert(key.to_owned(), raw_val.as_ref().to_owned());
        }

        let environment = match map.get("TANKHOR_ENV").map(String::as_str) {
            None => return Err(ConfigError::MissingEnvironment),
            Some("development") => RuntimeEnvironment::Development,
            Some("staging") => RuntimeEnvironment::Staging,
            Some("production") => RuntimeEnvironment::Production,
            Some(_) => return Err(ConfigError::InvalidEnvironment),
        };

        let adapter_mode = match map.get("TANKHOR_ADAPTERS").map(String::as_str) {
            None | Some("real") => AdapterMode::Real,
            Some("fake") => AdapterMode::Fake,
            Some(_) => return Err(ConfigError::InvalidAdapterMode),
        };

        if environment == RuntimeEnvironment::Production && adapter_mode == AdapterMode::Fake {
            return Err(ConfigError::ProductionRejectsFakeAdapters);
        }

        Ok(Self {
            environment,
            adapter_mode,
        })
    }

    /// Validates configuration strictly before invoking any backend resource acquisition.
    ///
    /// # Errors
    ///
    /// Returns a validation or acquisition error, guaranteeing `acquire` is never called
    /// when configuration validation fails.
    pub fn load_and_acquire<I, K, V, T, E, F>(pairs: I, acquire: F) -> Result<T, E>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
        E: From<ConfigError>,
        F: FnOnce(Self) -> Result<T, E>,
    {
        let config = Self::from_pairs(pairs)?;
        acquire(config)
    }

    /// Returns whether a CLI subcommand requires runtime adapter/transport configuration.
    #[must_use]
    pub fn command_requires_runtime_config(command: &str) -> bool {
        command != "openapi"
    }

    #[must_use]
    pub const fn environment(self) -> RuntimeEnvironment {
        self.environment
    }

    #[must_use]
    pub const fn adapter_mode(self) -> AdapterMode {
        self.adapter_mode
    }
}

fn is_per_group_adapter_key(key: &str) -> bool {
    if key == "TANKHOR_ADAPTERS" || !key.starts_with("TANKHOR_") {
        return false;
    }
    key.starts_with("TANKHOR_ADAPTERS_") || key.contains("_ADAPTER")
}
