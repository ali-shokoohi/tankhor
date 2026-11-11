use admin::domain::{
    admin_state::{AdminState, Eligibility, SecurityEpoch},
    audit::Factor,
    auth_state::BindingDigest,
    session_state::{SessionInput, SessionState},
};
pub fn now() -> time::OffsetDateTime {
    time::OffsetDateTime::UNIX_EPOCH
}
pub fn admin(n: u8) -> AdminState {
    AdminState::new(
        format!("019a53a0-0000-7000-8000-{n:012}").parse().unwrap(),
        Eligibility::Active,
        SecurityEpoch::new(0),
    )
}
pub fn digest() -> BindingDigest {
    BindingDigest::new([7; 32])
}
pub fn session(complete: bool) -> SessionState {
    SessionState::try_from(SessionInput {
        id: "019a53a0-0000-7000-8000-000000000004".parse().unwrap(),
        admin_id: admin(1).id(),
        epoch: admin(1).epoch(),
        primary_verified: true,
        factor: complete.then_some(Factor::Totp),
        issued_at: now(),
        expires_at: now() + time::Duration::minutes(5),
        idle_timeout: time::Duration::MINUTE,
    })
    .unwrap()
}
