use admin::domain::{
    admin_state::{AdminState, Eligibility, EnrollmentPurpose, Invalidation, SecurityEpoch},
    audit::Factor,
    auth_state::BindingDigest,
    session_state::{SessionInput, SessionState},
};
pub fn now() -> time::OffsetDateTime {
    time::macros::datetime!(2026-01-01 00:00 UTC)
}
pub fn actor() -> AdminState {
    AdminState::new(id(9), Eligibility::Active, SecurityEpoch::new(2))
}
pub fn id(n: u8) -> kernel::ids::AdminId {
    format!("019a53a0-0000-7000-8000-{n:012}").parse().unwrap()
}
pub fn target() -> AdminState {
    let mut state = AdminState::new(id(1), Eligibility::Active, SecurityEpoch::new(4));
    state
        .invalidate(
            SecurityEpoch::new(4),
            SecurityEpoch::new(5),
            Invalidation::Recover,
        )
        .unwrap();
    state
        .authorize_enrollment(
            &actor(),
            EnrollmentPurpose::Recovery,
            state.epoch(),
            now(),
            now() + time::Duration::minutes(5),
        )
        .unwrap();
    state.restore(state.epoch()).unwrap();
    state
}
pub fn digest(n: u8) -> BindingDigest {
    BindingDigest::new([n; 32])
}
pub fn session(admin: &AdminState, n: u8, complete: bool) -> SessionState {
    SessionState::try_from(SessionInput {
        id: format!("019a53a0-0000-7000-8000-{n:012}").parse().unwrap(),
        admin_id: admin.id(),
        epoch: admin.epoch(),
        primary_verified: true,
        factor: complete.then_some(Factor::Passkey),
        issued_at: now(),
        expires_at: now() + time::Duration::minutes(5),
        idle_timeout: time::Duration::MINUTE,
    })
    .unwrap()
}
#[path = "otp_policy.rs"]
mod shared_policy;
pub use shared_policy::policy;
