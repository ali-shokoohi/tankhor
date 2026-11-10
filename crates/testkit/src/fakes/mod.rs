//! In-memory deterministic fakes gated behind the `fakes` Cargo feature.

pub mod clock;
pub mod health_probe;
pub mod id_generator;
pub mod store;
pub mod unit_of_work;
