//! Assembly of tested fixture lifecycle components; no independent policy.

use super::lease::{LeaseError, SetupLock};
use std::path::Path;

/// Claim the live run before any shared service startup/reuse; retain this setup lock
/// until readiness and export finish. The persisted lease outlives the setup process.
///
/// # Errors
/// Returns the tested lock/lease refusal for foreign, busy or unverifiable state.
pub fn claim_run(
    directory: &Path,
    project: &str,
    run: &str,
    owner: u32,
) -> Result<SetupLock, LeaseError> {
    let lock = SetupLock::acquire(directory)?;
    lock.claim(directory, project, run, owner)?;
    Ok(lock)
}
