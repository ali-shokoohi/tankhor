#[path = "enrollment_fixture.rs"]
mod base;
use admin::{
    domain::{
        admin_state::AdminState,
        auth_state::{AuthInput, AuthState},
    },
    ports::session_store::{PendingCheck, PendingRecord},
};
pub use base::{policy, secret};
pub fn now() -> time::OffsetDateTime {
    time::macros::datetime!(2026-01-01 00:00 UTC)
}
pub fn target(authorized: bool) -> AdminState {
    // Preserve the shared identity fixture while independently advancing authorization.
    let mut target = base::target(false);
    if authorized {
        let actor = AdminState::new(
            "019a53a0-0000-7000-8000-000000000002".parse().unwrap(),
            admin::domain::admin_state::Eligibility::Active,
            target.epoch(),
        );
        target
            .authorize_enrollment(
                &actor,
                admin::domain::admin_state::EnrollmentPurpose::Addition,
                target.epoch(),
                now(),
                now() + time::Duration::minutes(5),
            )
            .unwrap();
    }
    target
}
pub fn check() -> PendingCheck {
    let mut check = base::check();
    check.now = now();
    check
}
pub fn pending() -> PendingRecord {
    PendingRecord::new(
        check().reference,
        AuthState::try_from(AuthInput {
            id: base::pending().state().id(),
            binding: check().binding,
            primary_verified_at: Some(now()),
            created_at: now(),
            expires_at: now() + time::Duration::MINUTE,
        })
        .unwrap(),
        None,
        None,
    )
    .unwrap()
}
