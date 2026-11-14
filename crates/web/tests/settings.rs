mod support;

use std::str::FromStr;

use admin::{
    application::seed_foundation::{SeedFoundation, SeedFoundationInput},
    domain::{
        change_revision::{ChangeSubject, ChangeValue},
        formats::{FormatSet, ImageFormat},
        settings::{SettingKey, SettingValue},
    },
    ports::{
        change_repository::ChangeRepository,
        settings_repository::{SettingVersion, SettingsRepository},
    },
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use kernel::{
    ids::{AdminId, AuditId, SessionId, VersionId},
    ports::{clock::Clock, unit_of_work::TransactionMode},
};
use support::{CSRF, Fixture, ORIGIN, seed_console_session};
use testkit::fakes::{
    change_repository::FakeChangeRepository, flag_repository::FakeFlagRepository,
    settings_repository::FakeSettingsRepository,
};
use time::Duration;
use tower::ServiceExt;
use web::{
    console::settings::settings_router,
    security::{CookieSecurityPolicy, CsrfContext},
};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn settings_forms_preserve_input_and_report_bounds_and_conflicts() {
    let fixture = Fixture::new();
    let actor = AdminId::from_str("019a53a0-0000-7000-8000-000000000041").unwrap();
    let second_actor = AdminId::from_str("019a53a0-0000-7000-8000-000000000042").unwrap();
    let session = SessionId::from_str("019a53a0-0000-7000-8000-000000000043").unwrap();
    let second_session = SessionId::from_str("019a53a0-0000-7000-8000-000000000044").unwrap();
    let token = seed_console_session(&fixture, actor, session).await;
    let second_token = seed_console_session(&fixture, second_actor, second_session).await;

    let settings_repo = FakeSettingsRepository::new(fixture.store.clone());
    let flags_repo = FakeFlagRepository::new(fixture.store.clone());
    let changes_repo = FakeChangeRepository::new(fixture.store.clone());

    let seeder = SeedFoundation {
        uow: fixture.services.uow.as_ref(),
        settings: &settings_repo,
        flags: &flags_repo,
        audit: fixture.services.audit.as_ref(),
        clock: fixture.services.clock.as_ref(),
        ids: fixture.services.ids.as_ref(),
    };
    seeder
        .execute(SeedFoundationInput {
            operation: AuditId::from_str("019a53a0-0000-7000-8000-000000000045").unwrap(),
            system: actor,
        })
        .await
        .unwrap();

    // Advance upload_max_mb once so detail history has two versions (15 -> 20).
    let mut tx = fixture
        .services
        .uow
        .begin(TransactionMode::ReadWrite)
        .await
        .unwrap();
    let current = settings_repo
        .get(&mut tx, SettingKey::UploadMaxMb)
        .await
        .unwrap()
        .unwrap()
        .version()
        .version();
    let next_version_id = VersionId::from_str("019a53a0-0000-7000-8000-000000000046").unwrap();
    let next_version = SettingVersion::new(
        next_version_id,
        SettingKey::UploadMaxMb,
        SettingValue::Number(20),
        actor,
        fixture.clock.now().await.unwrap() + Duration::seconds(10),
    )
    .unwrap();
    settings_repo
        .append_version(&mut tx, next_version)
        .await
        .unwrap();
    settings_repo
        .set_live(
            &mut tx,
            SettingKey::UploadMaxMb,
            Some(current),
            next_version_id,
        )
        .await
        .unwrap();
    fixture.services.uow.commit(tx).await.unwrap();

    let cookies = CookieSecurityPolicy::production();
    let app = settings_router(fixture.services.clone(), fixture.csrf(), cookies);

    // 1. Unauthenticated request is rejected with 401 and no-store.
    let denied = app
        .clone()
        .oneshot(
            Request::get("/console/settings?lang=en")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(denied.headers()[header::CACHE_CONTROL], "no-store");

    // 2. GET /console/settings lists all 10 typed keys with localized labels, live values, units, and bounds.
    let all_keys = [
        SettingKey::BuyerTempPhotoDays,
        SettingKey::StudioTempUploadDays,
        SettingKey::StudioAutoDraftDays,
        SettingKey::AccountRecoveryDays,
        SettingKey::StudioMaxCards,
        SettingKey::BuyerMaxPermanentPhotos,
        SettingKey::UploadMaxMb,
        SettingKey::UploadMinShortestEdgePx,
        SettingKey::GenerationMaxAttempts,
        SettingKey::UploadFormats,
    ];
    let list_en = app
        .clone()
        .oneshot(
            Request::get("/console/settings?lang=en")
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_en.status(), StatusCode::OK);
    assert_eq!(list_en.headers()[header::CACHE_CONTROL], "no-store");
    let list_en_html = String::from_utf8(
        to_bytes(list_en.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(list_en_html.contains("lang=\"en\""));
    assert!(list_en_html.contains("dir=\"ltr\""));
    for key in all_keys {
        assert!(
            list_en_html.contains(key.as_str()),
            "list view must include setting key {}",
            key.as_str()
        );
        assert!(
            list_en_html.contains(&format!("/console/settings/{}", key.as_str())),
            "list view must link to detail route for {}",
            key.as_str()
        );
    }
    assert!(list_en_html.contains("1..=25"));

    // Persian list view renders RTL and Persian labels while keeping canonical setting identifiers intact.
    let list_fa = app
        .clone()
        .oneshot(
            Request::get("/console/settings?lang=fa")
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_fa.status(), StatusCode::OK);
    let list_fa_html = String::from_utf8(
        to_bytes(list_fa.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(list_fa_html.contains("lang=\"fa\""));
    assert!(list_fa_html.contains("dir=\"rtl\""));
    assert!(list_fa_html.contains("upload_max_mb"));

    // 3. GET /console/settings/upload_max_mb renders live value (20), unit, bounds (1..=25), version history (20 and 15), and draft form.
    // No arbitrary registry policy editing fields (minimum, maximum, unit, new key creation) may exist.
    let detail_en = app
        .clone()
        .oneshot(
            Request::get("/console/settings/upload_max_mb?lang=en")
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_en.status(), StatusCode::OK);
    assert_eq!(detail_en.headers()[header::CACHE_CONTROL], "no-store");
    let detail_html = String::from_utf8(
        to_bytes(detail_en.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(detail_html.contains("upload_max_mb"));
    assert!(detail_html.contains("20"));
    assert!(detail_html.contains("15"));
    assert!(detail_html.contains("019a53a0-0000-7000-8000-000000000046"));
    assert!(detail_html.contains(&actor.to_string()));
    assert!(detail_html.contains("action=\"/console/settings/upload_max_mb/drafts\""));
    assert!(!detail_html.contains("name=\"minimum\""));
    assert!(!detail_html.contains("name=\"maximum\""));
    assert!(!detail_html.contains("name=\"unit\""));

    // Unknown setting key returns 404 without panic.
    let unknown_detail = app
        .clone()
        .oneshot(
            Request::get("/console/settings/unknown_setting_key?lang=en")
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_detail.status(), StatusCode::NOT_FOUND);

    // 4. Out-of-bounds integer submission (upload_max_mb = 99 > 25) returns 422, localized field-linked error,
    // preserves submitted input ("99") and schedule input, and leaves live value at 20 with 0 saved drafts.
    let invalid_bounds_en = app
        .clone()
        .oneshot(
            Request::post("/console/settings/upload_max_mb/drafts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "value=99&local_date=2026-05-01&local_time=14%3A30&time_zone=Asia%2FTehran&csrf_token={CSRF}&lang=en"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_bounds_en.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid_bounds_en.headers()[header::CACHE_CONTROL],
        "no-store"
    );
    let invalid_bounds_en_html = String::from_utf8(
        to_bytes(invalid_bounds_en.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(invalid_bounds_en_html.contains("value=\"99\""));
    assert!(invalid_bounds_en_html.contains("2026-05-01"));
    assert!(invalid_bounds_en_html.contains("14:30"));
    assert!(invalid_bounds_en_html.contains("Asia/Tehran"));
    assert!(invalid_bounds_en_html.contains("aria-invalid=\"true\""));
    assert!(invalid_bounds_en_html.contains("aria-describedby="));
    assert!(invalid_bounds_en_html.contains('1') && invalid_bounds_en_html.contains("25"));

    // Same bounds failure in Persian (lang=fa) renders localized Persian error and preserves input.
    let invalid_bounds_fa = app
        .clone()
        .oneshot(
            Request::post("/console/settings/upload_max_mb/drafts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!("value=99&csrf_token={CSRF}&lang=fa")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_bounds_fa.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_bounds_fa_html = String::from_utf8(
        to_bytes(invalid_bounds_fa.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(invalid_bounds_fa_html.contains("lang=\"fa\""));
    assert!(invalid_bounds_fa_html.contains("dir=\"rtl\""));
    assert!(invalid_bounds_fa_html.contains("value=\"99\""));
    assert!(invalid_bounds_fa_html.contains("aria-invalid=\"true\""));

    // 5. UploadFormats submission without mandatory JPEG returns 422, localized error, and preserves selected formats.
    let invalid_formats = app
        .clone()
        .oneshot(
            Request::post("/console/settings/upload_formats/drafts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "formats=PNG&formats=WEBP&csrf_token={CSRF}&lang=en"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_formats.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_formats_html = String::from_utf8(
        to_bytes(invalid_formats.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(invalid_formats_html.contains("JPEG"));
    assert!(invalid_formats_html.contains("aria-invalid=\"true\""));
    assert!(invalid_formats_html.contains("value=\"PNG\" checked"));
    assert!(invalid_formats_html.contains("value=\"WEBP\" checked"));

    // Verify no drafts were persisted after invalid attempts and live values remain unchanged.
    {
        let state = fixture.store.try_read_committed().unwrap();
        assert!(state.changes.is_empty());
        assert_eq!(
            state.settings[&SettingKey::UploadMaxMb].version().value(),
            &SettingValue::Number(20)
        );
        assert_eq!(
            state.settings[&SettingKey::UploadFormats].version().value(),
            &SettingValue::Formats(FormatSet::all())
        );
    }

    // 6. Valid draft submission creates a draft only, leaves live value at 20, and redirects (303) to /console/changes/{id}.
    let create_draft = app
        .clone()
        .oneshot(
            Request::post("/console/settings/upload_max_mb/drafts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!("value=22&csrf_token={CSRF}&lang=en")))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_draft.status(), StatusCode::SEE_OTHER);
    assert_eq!(create_draft.headers()[header::CACHE_CONTROL], "no-store");
    let location = create_draft.headers()[header::LOCATION]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        location.starts_with("/console/changes/"),
        "expected redirect to /console/changes/{{id}}, got {location}"
    );

    let (change_id, first_head) = {
        let mut tx = fixture
            .services
            .uow
            .begin(TransactionMode::ReadOnly)
            .await
            .unwrap();
        let live = settings_repo
            .get(&mut tx, SettingKey::UploadMaxMb)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(live.version().value(), &SettingValue::Number(20));

        let stored_id = {
            let state = fixture.store.try_read_committed().unwrap();
            assert_eq!(state.changes.len(), 1);
            *state.changes.keys().next().unwrap()
        };
        let draft = changes_repo
            .draft(&mut tx, stored_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(draft.creator(), actor);
        assert_eq!(
            draft.subject(),
            ChangeSubject::Setting(SettingKey::UploadMaxMb)
        );
        let head = draft.head().unwrap();
        let rev = draft.current().unwrap();
        assert_eq!(rev.created_by(), actor);
        assert_eq!(rev.value(), ChangeValue::Number(22));
        fixture.services.uow.rollback(tx).await.unwrap();
        (stored_id, head)
    };
    assert!(location.contains(&change_id.to_string()));

    // Valid UploadFormats draft submission with JPEG + PNG succeeds with 303.
    let create_formats_draft = app
        .clone()
        .oneshot(
            Request::post("/console/settings/upload_formats/drafts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "formats=JPEG&formats=PNG&csrf_token={CSRF}&lang=en"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_formats_draft.status(), StatusCode::SEE_OTHER);
    {
        let state = fixture.store.try_read_committed().unwrap();
        let formats_draft = state
            .changes
            .values()
            .find(|d| d.subject() == ChangeSubject::Setting(SettingKey::UploadFormats))
            .unwrap();
        assert_eq!(
            formats_draft.current().unwrap().value(),
            ChangeValue::Formats(
                FormatSet::from_mask(ImageFormat::Jpeg.bit() | ImageFormat::Png.bit()).unwrap()
            )
        );
    }

    // 7. Second actor appends revision 2 (24) using expected_head = first_head -> 303.
    let update_draft = app
        .clone()
        .oneshot(
            Request::post("/console/settings/upload_max_mb/drafts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={second_token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "change={change_id}&expected_head={first_head}&value=24&csrf_token={CSRF}&lang=en"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_draft.status(), StatusCode::SEE_OTHER);

    // 8. First actor submits stale expected_head = first_head with value=25 -> HTTP 409 CONFLICT,
    // localized conflict message, preserved input ("25"), and revision 2 remains intact at 24.
    let stale_conflict = app
        .oneshot(
            Request::post("/console/settings/upload_max_mb/drafts")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ORIGIN, ORIGIN)
                .header(
                    header::COOKIE,
                    format!("__Host-tankhor_admin_session={token}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "change={change_id}&expected_head={first_head}&value=25&csrf_token={CSRF}&lang=en"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale_conflict.status(), StatusCode::CONFLICT);
    assert_eq!(stale_conflict.headers()[header::CACHE_CONTROL], "no-store");
    let stale_html = String::from_utf8(
        to_bytes(stale_conflict.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(stale_html.contains("value=\"25\""));
    assert!(stale_html.contains("role=\"alert\""));

    let mut tx = fixture
        .services
        .uow
        .begin(TransactionMode::ReadOnly)
        .await
        .unwrap();
    let draft = changes_repo
        .draft(&mut tx, change_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(draft.revisions().len(), 2);
    assert_eq!(draft.current().unwrap().value(), ChangeValue::Number(24));
    assert_eq!(draft.current().unwrap().created_by(), second_actor);
    fixture.services.uow.rollback(tx).await.unwrap();
}
