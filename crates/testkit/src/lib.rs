//! Test-only fixtures, shared contracts and explicitly enabled in-memory fakes.
//! Production crates must never depend on this package.

pub mod contracts;
#[cfg(feature = "fakes")]
pub mod fakes;
pub mod fixtures;
