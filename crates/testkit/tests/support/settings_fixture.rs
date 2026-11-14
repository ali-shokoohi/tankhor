use admin::{
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        audit::Factor,
        auth_state::BindingDigest,
        flag::launch_flags,
        session_state::{SessionInput, SessionState},
        settings::{SettingValue, initial_numeric_settings, initial_upload_formats},
    },
    ports::{flag_repository::FlagVersion, settings_repository::SettingVersion},
};
use kernel::ids::{AdminId, VersionId};
use std::str::FromStr;
use time::{OffsetDateTime, macros::datetime};

pub fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-6200-7000-8000-{n:012x}")
        .parse()
        .ok()
        .unwrap()
}
pub fn now() -> OffsetDateTime {
    datetime!(2026-01-01 00:00 UTC)
}
pub fn admin() -> AdminState {
    AdminState::new(id::<AdminId>(1), Eligibility::Active, SecurityEpoch::new(0))
}
pub fn digest() -> BindingDigest {
    BindingDigest::new([23; 32])
}
pub fn session() -> SessionState {
    SessionState::try_from(SessionInput {
        id: id(2),
        admin_id: admin().id(),
        epoch: admin().epoch(),
        primary_verified: true,
        factor: Some(Factor::Totp),
        issued_at: now(),
        expires_at: now() + time::Duration::minutes(5),
        idle_timeout: time::Duration::MINUTE,
    })
    .unwrap()
}
#[allow(dead_code)]
pub fn versions() -> Vec<SettingVersion> {
    initial_numeric_settings()
        .into_iter()
        .enumerate()
        .map(|(index, setting)| {
            SettingVersion::new(
                id::<VersionId>(u16::try_from(index).unwrap() + 10),
                setting.key(),
                SettingValue::Number(i128::from(setting.value())),
                admin().id(),
                now(),
            )
            .unwrap()
        })
        .chain([SettingVersion::new(
            id::<VersionId>(19),
            initial_upload_formats().key(),
            SettingValue::Formats(initial_upload_formats().value()),
            admin().id(),
            now(),
        )
        .unwrap()])
        .collect()
}
#[allow(dead_code)]
pub fn flag_versions() -> Vec<FlagVersion> {
    launch_flags()
        .into_iter()
        .enumerate()
        .map(|(index, definition)| {
            FlagVersion::new(
                id::<VersionId>(u16::try_from(index).unwrap() + 30),
                definition.key(),
                false,
                admin().id(),
                now(),
            )
        })
        .collect()
}
