use admin::domain::{
    admin_state::{AdminState, Eligibility, SecurityEpoch},
    audit::Factor,
    auth_state::BindingDigest,
    secrets::SecretBytes,
    session_state::{SessionInput, SessionState},
};
pub fn now() -> time::OffsetDateTime {
    time::OffsetDateTime::UNIX_EPOCH
}
pub fn actor() -> AdminState {
    AdminState::new(
        "019a53a0-0000-7000-8000-000000000001".parse().unwrap(),
        Eligibility::Active,
        SecurityEpoch::new(0),
    )
}
pub fn target() -> kernel::ids::AdminId {
    "019a53a0-0000-7000-8000-000000000002".parse().unwrap()
}
pub fn digest() -> BindingDigest {
    BindingDigest::new([7; 32])
}
pub fn phone() -> SecretBytes {
    SecretBytes::new(b"+989120000002".to_vec())
}
pub fn session(complete: bool) -> SessionState {
    SessionState::try_from(SessionInput {
        id: "019a53a0-0000-7000-8000-000000000004".parse().unwrap(),
        admin_id: actor().id(),
        epoch: actor().epoch(),
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
