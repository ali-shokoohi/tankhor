//! Checked `UUIDv7` identifier wrappers shared across Tankhor contexts.
//!
//! Browser capability tokens are random opaque secrets, not `UUIDv7` identifiers.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use uuid::{Uuid, Version};

/// Error returned when an external identifier is malformed or not UUID version 7.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IdError {
    /// Input string is not a valid UUID representation.
    #[error("invalid identifier syntax")]
    InvalidSyntax,
    /// Input UUID does not carry version 7 and RFC 9562 variant bits.
    #[error("identifier must be UUID version 7")]
    InvalidVersion,
}

fn validate_uuid_v7(value: Uuid) -> Result<Uuid, IdError> {
    if value.get_version() == Some(Version::SortRand)
        && value.get_variant() == uuid::Variant::RFC4122
    {
        Ok(value)
    } else {
        Err(IdError::InvalidVersion)
    }
}

macro_rules! define_uuid_v7_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Uuid);

        impl $name {
            /// Construct a checked `UUIDv7` wrapper from a raw [`Uuid`].
            ///
            /// # Errors
            ///
            /// Returns [`IdError::InvalidVersion`] if `value` is not RFC 9562 `UUIDv7`.
            pub fn try_new(value: Uuid) -> Result<Self, IdError> {
                validate_uuid_v7(value).map(Self)
            }

            /// Return the underlying [`Uuid`] value.
            #[must_use]
            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl TryFrom<Uuid> for $name {
            type Error = IdError;

            fn try_from(value: Uuid) -> Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = IdError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let parsed = Uuid::parse_str(s).map_err(|_| IdError::InvalidSyntax)?;
                Self::try_new(parsed)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                self.0.serialize(serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = Uuid::deserialize(deserializer)?;
                Self::try_new(raw).map_err(serde::de::Error::custom)
            }
        }
    };
}

define_uuid_v7_id!(
    /// Provisioned administrator identity.
    AdminId
);
define_uuid_v7_id!(
    /// Enrolled second-factor credential identity.
    CredentialId
);
define_uuid_v7_id!(
    /// Server-side administrator session identity.
    SessionId
);
define_uuid_v7_id!(
    /// Primary or second-factor challenge/ceremony identity.
    ChallengeId
);
define_uuid_v7_id!(
    /// Configuration or flag change draft identity.
    ChangeId
);
define_uuid_v7_id!(
    /// Immutable draft revision identity.
    RevisionId
);
define_uuid_v7_id!(
    /// Immutable preview assessment identity.
    PreviewId
);
define_uuid_v7_id!(
    /// Immutable exact-revision approval identity.
    ApprovalId
);
define_uuid_v7_id!(
    /// Publication or scheduled-publication request identity.
    PublicationId
);
define_uuid_v7_id!(
    /// Immutable published setting or flag version identity.
    VersionId
);
define_uuid_v7_id!(
    /// Immutable audit event identity.
    AuditId
);
