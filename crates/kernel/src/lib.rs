//! Shared kernel: pure identifier, locale and error types plus the Clock, `IdGenerator`
//! and `UnitOfWork` ports used by every context. It performs no I/O.

pub mod error;
pub mod ids;
pub mod locale;
pub mod ports;
