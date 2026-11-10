//! Tankhor composition root crate.

#[cfg(all(feature = "dev-fakes", not(debug_assertions)))]
compile_error!("feature `dev-fakes` is debug-only and forbidden in release builds");

pub mod composition;
pub mod config;
pub mod foundation;
pub mod openapi;
pub mod serve;
pub mod transport_policy;
