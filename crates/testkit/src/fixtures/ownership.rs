//! Exact fixture ownership and endpoint identity (T012; FR-024/FR-CON-37).

use std::{collections::BTreeMap, net::IpAddr};

/// Typed refusal without copying resource metadata or credentials into errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipError {
    InvalidScope,
    MissingLabel,
    MismatchedLabel,
    NonLoopback,
    InvalidPort,
    WrongEndpoint,
}

impl std::fmt::Display for OwnershipError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "fixture ownership refusal: {self:?}")
    }
}

impl std::error::Error for OwnershipError {}

/// A literal loopback address and concrete published port; DNS is never resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    host: IpAddr,
    port: u16,
}

impl Endpoint {
    /// Validate an address before it can be used for fixture access.
    ///
    /// # Errors
    /// Rejects DNS/non-loopback addresses and port zero.
    pub fn new(host: &str, port: u16) -> Result<Self, OwnershipError> {
        let host: IpAddr = host.parse().map_err(|_| OwnershipError::NonLoopback)?;
        if !host.is_loopback() {
            return Err(OwnershipError::NonLoopback);
        }
        if port == 0 {
            return Err(OwnershipError::InvalidPort);
        }
        Ok(Self { host, port })
    }
}

/// Exact repository and lifecycle/run scope. Developer stacks have no fixture identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureScope {
    project: String,
    run: String,
}

impl FixtureScope {
    /// Construct identifiers safe for labels and local fixture paths.
    ///
    /// # Errors
    /// Rejects empty identifiers or characters outside ASCII alphanumeric, dash and underscore.
    pub fn new(project: &str, run: &str) -> Result<Self, OwnershipError> {
        for identifier in [project, run] {
            if identifier.is_empty()
                || !identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(OwnershipError::InvalidScope);
            }
        }
        Ok(Self {
            project: project.to_owned(),
            run: run.to_owned(),
        })
    }

    /// Required ownership labels; other container metadata is not an ownership grant.
    #[must_use]
    pub fn postgres_labels(&self) -> BTreeMap<String, String> {
        [
            ("io.tankhor.fixture", "true"),
            ("io.tankhor.fixture.project", self.project.as_str()),
            ("io.tankhor.fixture.run", self.run.as_str()),
            ("io.tankhor.fixture.service", "postgres"),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect()
    }

    /// Compare labels and the requested address with the inspected resource's endpoint.
    ///
    /// # Errors
    /// Refuses missing/mismatched identity or a different published endpoint.
    pub fn validate(
        &self,
        labels: &BTreeMap<String, String>,
        requested: Endpoint,
        inspected: Endpoint,
    ) -> Result<(), OwnershipError> {
        for (key, expected) in self.postgres_labels() {
            let actual = labels.get(&key).ok_or(OwnershipError::MissingLabel)?;
            if actual != &expected {
                return Err(OwnershipError::MismatchedLabel);
            }
        }
        if requested != inspected {
            return Err(OwnershipError::WrongEndpoint);
        }
        Ok(())
    }
}
