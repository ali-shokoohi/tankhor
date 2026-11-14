//! HTTP contract tests for change review and impact preview routes (`crates/web/src/console/changes.rs`).
//! T361: FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41,
//! AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046.

mod support;

use admin::{
    application::{
        preview::{Preview, PreviewEvidence, PreviewInput},
        save_draft::{SaveDraft, SaveDraftInput},
        seed_foundation::{SeedFoundation, SeedFoundationInput},
    },
    domain::{
        audit::ReasonCode,
        change_revision::{ChangeSubject, ChangeValue},
        formats::FormatSet,
        impact_attempts::{GenerationUnitSnapshot, UnitQueueState},
        impact_cards::CardProjectSnapshot,
        impact_lifetime::{LifetimeItem, LifetimePopulation},
        impact_photos::PhotoAccountSnapshot,
        impact_recovery::PendingRecoverySnapshot,
        impact_uploads::UploadPolicySnapshot,
        secrets::SecretBytes,
        settings::SettingKey,
    },
    ports::admin_authenticator::TokenPurpose,
};
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use kernel::ids::{AdminId, AuditId, SessionId};
use support::{CSRF, Fixture, ORIGIN, seed_console_session};
use time::Duration;
use tower::ServiceExt;
use web::{
    console::changes::changes_router,
    security::{CookieSecurityPolicy, CsrfContext},
};

fn id(n: u16) -> String {
    format!("019a53a0-0000-7000-8000-{n:012x}")
}

async fn seed_registry(fixture: &Fixture, system_admin: AdminId) {
    let seed = SeedFoundation {
        uow: fixture.services.uow.as_ref(),
        settings: fixture.services.settings.as_ref(),
        flags: fixture.services.flags.as_ref(),
        audit: fixture.services.audit.as_ref(),
        clock: fixture.services.clock.as_ref(),
        ids: fixture.services.ids.as_ref(),
    };
    seed.execute(SeedFoundationInput {
        operation: id(950).parse::<AuditId>().unwrap(),
        system: system_admin,
    })
    .await
    .unwrap();
}

async fn create_setting_draft(
    fixture: &Fixture,
    cookie_token: &str,
    key: SettingKey,
    value: ChangeValue,
) -> admin::domain::change_revision::ChangeDraft {
    let actor = fixture
        .services
        .authenticator
        .token_digest(
            TokenPurpose::Session,
            SecretBytes::new(cookie_token.as_bytes().to_vec()),
        )
        .await
        .unwrap();
    let save = SaveDraft {
        uow: fixture.services.uow.as_ref(),
        sessions: fixture.services.sessions.as_ref(),
        admins: fixture.services.admins.as_ref(),
        settings: fixture.services.settings.as_ref(),
        flags: fixture.services.flags.as_ref(),
        changes: fixture.services.changes.as_ref(),
        audit: fixture.services.audit.as_ref(),
        clock: fixture.services.clock.as_ref(),
        ids: fixture.services.ids.as_ref(),
    };
    save.execute(SaveDraftInput {
        actor,
        change: None,
        subject: ChangeSubject::Setting(key),
        value,
        reason: ReasonCode::DraftChange,
        expected_head: None,
        schedule: None,
    })
    .await
    .unwrap()
}

async fn run_service_preview(
    fixture: &Fixture,
    cookie_token: &str,
    change_id: &str,
    revision_id: &str,
    evidence: PreviewEvidence,
) {
    let actor = fixture
        .services
        .authenticator
        .token_digest(
            TokenPurpose::Session,
            SecretBytes::new(cookie_token.as_bytes().to_vec()),
        )
        .await
        .unwrap();
    let preview = Preview {
        uow: fixture.services.uow.as_ref(),
        sessions: fixture.services.sessions.as_ref(),
        admins: fixture.services.admins.as_ref(),
        settings: fixture.services.settings.as_ref(),
        flags: fixture.services.flags.as_ref(),
        changes: fixture.services.changes.as_ref(),
        audit: fixture.services.audit.as_ref(),
        clock: fixture.services.clock.as_ref(),
        ids: fixture.services.ids.as_ref(),
    };
    preview
        .execute(PreviewInput {
            actor,
            change: change_id.to_owned(),
            revision: revision_id.to_owned(),
            evidence,
        })
        .await
        .unwrap();
}

#[tokio::test]
#[allow(clippy::similar_names, clippy::too_many_lines)]
async fn preview_views_explain_every_preservation_policy_and_unknown_count() {
    let fixture = Fixture::new();
    let actor = id(901).parse::<AdminId>().unwrap();
    let session = id(902).parse::<SessionId>().unwrap();
    let cookie = seed_console_session(&fixture, actor, session).await;
    let now = time::macros::datetime!(2026-04-04 12:00 UTC);
    seed_registry(&fixture, actor).await;

    let app = changes_router(
        fixture.services.clone(),
        fixture.csrf(),
        CookieSecurityPolicy::loopback_insecure_dev(),
    );

    // 1. Shorter lifetime with KNOWN positive counts (buyer_temp_photo_days: 14 -> 7).
    let lifetime_known_draft = create_setting_draft(
        &fixture,
        &cookie,
        SettingKey::BuyerTempPhotoDays,
        ChangeValue::Number(7),
    )
    .await;
    let lk_id = lifetime_known_draft.id().to_string();
    let lk_rev = lifetime_known_draft.head().unwrap().to_string();
    let lk_base = lifetime_known_draft.baseline_version().to_string();
    run_service_preview(
        &fixture,
        &cookie,
        &lk_id,
        &lk_rev,
        PreviewEvidence::Lifetime(LifetimePopulation::Snapshots(vec![
            // Uploaded 10 days ago (current expiry +4d, new expiry -3d -> already due on next sweep)
            LifetimeItem::new(now - Duration::days(10), now + Duration::days(4)),
            // Uploaded 3 days ago (current expiry +11d, new expiry +4d -> changed count)
            LifetimeItem::new(now - Duration::days(3), now + Duration::days(11)),
        ])),
    )
    .await;

    // Unauthenticated GET /console/changes/{id} -> 401
    let unauth_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/console/changes/{lk_id}?lang=en"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth_resp.status(), StatusCode::UNAUTHORIZED);

    // Authenticated GET /console/changes/{id} in EN with mobile viewport
    let lk_en_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/console/changes/{lk_id}?lang=en&viewport=mobile"))
                .header(
                    header::COOKIE,
                    format!("tankhor_dev_admin_session={cookie}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(lk_en_resp.status(), StatusCode::OK);
    assert_eq!(
        lk_en_resp.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let lk_en_html = String::from_utf8(
        to_bytes(lk_en_resp.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(lk_en_html.contains("lang=\"en\""));
    assert!(lk_en_html.contains("dir=\"ltr\""));
    assert!(lk_en_html.contains(&lk_id));
    assert!(lk_en_html.contains(&lk_rev));
    assert!(lk_en_html.contains(&lk_base));
    // Shows live (14) and proposed (7) values
    assert!(lk_en_html.contains("14"));
    assert!(lk_en_html.contains('7'));
    // Viewport selector with mobile/desktop options and selected mobile viewport
    assert!(lk_en_html.contains("data-viewport=\"mobile\""));
    assert!(lk_en_html.contains("value=\"mobile\""));
    assert!(lk_en_html.contains("value=\"desktop\""));
    // Positive counts and next-sweep explanation
    assert!(lk_en_html.contains("changed_count"));
    assert!(lk_en_html.contains("already_due_count"));
    assert!(lk_en_html.contains("next owning lifecycle sweep"));
    assert!(lk_en_html.contains("original upload"));

    // Authenticated GET /console/changes/{id} in FA with desktop viewport
    let lk_fa_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/console/changes/{lk_id}?lang=fa&viewport=desktop"))
                .header(
                    header::COOKIE,
                    format!("tankhor_dev_admin_session={cookie}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(lk_fa_resp.status(), StatusCode::OK);
    let lk_fa_html = String::from_utf8(
        to_bytes(lk_fa_resp.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(lk_fa_html.contains("lang=\"fa\""));
    assert!(lk_fa_html.contains("dir=\"rtl\""));
    assert!(lk_fa_html.contains("data-viewport=\"desktop\""));
    assert!(lk_fa_html.contains("پایش چرخه حیات"));

    // 2. Shorter lifetime with UNKNOWN/UNAVAILABLE counts via POST /console/changes/{id}/preview
    // (studio_auto_draft_days: 30 -> 10). Unknown counts must remain explicitly unknown, never 0!
    let lifetime_unknown_draft = create_setting_draft(
        &fixture,
        &cookie,
        SettingKey::StudioAutoDraftDays,
        ChangeValue::Number(10),
    )
    .await;
    let lu_id = lifetime_unknown_draft.id().to_string();
    let lu_rev = lifetime_unknown_draft.head().unwrap().to_string();

    let post_preview_en = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/console/changes/{lu_id}/preview?lang=en"))
                .header(header::ORIGIN, ORIGIN)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("tankhor_dev_admin_session={cookie}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "csrf_token={CSRF}&lang=en&revision={lu_rev}&viewport=desktop"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(post_preview_en.status(), StatusCode::OK);
    let lu_en_html = String::from_utf8(
        to_bytes(post_preview_en.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(lu_en_html.contains("Unavailable (unknown)"));
    assert!(!lu_en_html.contains("changed_count: 0"));
    assert!(!lu_en_html.contains("already_due_count: 0"));
    assert!(lu_en_html.contains("last activity"));
    assert!(lu_en_html.contains("next owning lifecycle sweep"));

    let lu_fa_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/console/changes/{lu_id}?lang=fa"))
                .header(
                    header::COOKIE,
                    format!("tankhor_dev_admin_session={cookie}"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(lu_fa_resp.status(), StatusCode::OK);
    let lu_fa_html = String::from_utf8(
        to_bytes(lu_fa_resp.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(lu_fa_html.contains("نامشخص (خارج از دسترس)"));
    assert!(lu_fa_html.contains("آخرین فعالیت"));

    // 3. Stale revision on POST /console/changes/{id}/preview returns 409 Conflict and preserves revision input
    let stale_rev = id(999);
    let stale_preview_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/console/changes/{lu_id}/preview?lang=en"))
                .header(header::ORIGIN, ORIGIN)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("tankhor_dev_admin_session={cookie}"),
                )
                .extension(CsrfContext::new(CSRF))
                .body(Body::from(format!(
                    "csrf_token={CSRF}&lang=en&revision={stale_rev}&viewport=mobile"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale_preview_resp.status(), StatusCode::CONFLICT);
    let stale_html = String::from_utf8(
        to_bytes(stale_preview_resp.into_body(), 128 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(stale_html.contains("role=\"alert\""));
    assert!(stale_html.contains(&stale_rev));
    assert!(stale_html.contains("data-viewport=\"mobile\""));

    // 4. Card limit preservation (AC-ADM-039: studio_max_cards 50 -> 20)
    let card_draft = create_setting_draft(
        &fixture,
        &cookie,
        SettingKey::StudioMaxCards,
        ChangeValue::Number(20),
    )
    .await;
    let card_id = card_draft.id().to_string();
    let card_rev = card_draft.head().unwrap().to_string();
    run_service_preview(
        &fixture,
        &cookie,
        &card_id,
        &card_rev,
        PreviewEvidence::Card(CardProjectSnapshot::new(25, false)),
    )
    .await;
    for (lang, expected_phrase) in [
        (
            "en",
            "Existing projects are preserved and confirmed projects are unaffected",
        ),
        (
            "fa",
            "پروژه‌های موجود حفظ می‌شوند و پروژه‌های تأییدشده بدون تغییر می‌مانند",
        ),
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/console/changes/{card_id}?lang={lang}"))
                    .header(
                        header::COOKIE,
                        format!("tankhor_dev_admin_session={cookie}"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(resp.into_body(), 128 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            html.contains(expected_phrase),
            "missing {expected_phrase} in {lang}"
        );
    }

    // 5. Buyer permanent photo limit preservation (AC-ADM-040: buyer_max_permanent_photos 5 -> 3)
    let photo_draft = create_setting_draft(
        &fixture,
        &cookie,
        SettingKey::BuyerMaxPermanentPhotos,
        ChangeValue::Number(3),
    )
    .await;
    let photo_id = photo_draft.id().to_string();
    let photo_rev = photo_draft.head().unwrap().to_string();
    run_service_preview(
        &fixture,
        &cookie,
        &photo_id,
        &photo_rev,
        PreviewEvidence::Photo(PhotoAccountSnapshot::new(4)),
    )
    .await;
    for (lang, expected_phrase) in [
        (
            "en",
            "Existing permanent photos are preserved; new photos can be added only when below the new limit",
        ),
        (
            "fa",
            "عکس‌های دائمی موجود حفظ می‌شوند؛ افزودن عکس جدید فقط زمانی ممکن است که تعداد کمتر از سقف جدید باشد",
        ),
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/console/changes/{photo_id}?lang={lang}"))
                    .header(
                        header::COOKIE,
                        format!("tankhor_dev_admin_session={cookie}"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(resp.into_body(), 128 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            html.contains(expected_phrase),
            "missing {expected_phrase} in {lang}"
        );
    }

    // 6. Upload validation policy preservation (AC-ADM-041: upload_max_mb 15 -> 10)
    let upload_draft = create_setting_draft(
        &fixture,
        &cookie,
        SettingKey::UploadMaxMb,
        ChangeValue::Number(10),
    )
    .await;
    let upload_id = upload_draft.id().to_string();
    let upload_rev = upload_draft.head().unwrap().to_string();
    run_service_preview(
        &fixture,
        &cookie,
        &upload_id,
        &upload_rev,
        PreviewEvidence::Upload(UploadPolicySnapshot::new(15, 512, FormatSet::all())),
    )
    .await;
    for (lang, expected_phrase) in [
        (
            "en",
            "Stored files remain valid and untouched; the updated policy validates new uploads only",
        ),
        (
            "fa",
            "فایل‌های ذخیره‌شده معتبر و بدون تغییر باقی می‌مانند؛ سیاست جدید فقط بر بارگذاری‌های جدید اعمال می‌شود",
        ),
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/console/changes/{upload_id}?lang={lang}"))
                    .header(
                        header::COOKIE,
                        format!("tankhor_dev_admin_session={cookie}"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(resp.into_body(), 128 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            html.contains(expected_phrase),
            "missing {expected_phrase} in {lang}"
        );
    }

    // 7. Generation attempt limit preservation (AC-ADM-042: generation_max_attempts 3 -> 2)
    let attempt_draft = create_setting_draft(
        &fixture,
        &cookie,
        SettingKey::GenerationMaxAttempts,
        ChangeValue::Number(2),
    )
    .await;
    let attempt_id = attempt_draft.id().to_string();
    let attempt_rev = attempt_draft.head().unwrap().to_string();
    run_service_preview(
        &fixture,
        &cookie,
        &attempt_id,
        &attempt_rev,
        PreviewEvidence::Attempt(GenerationUnitSnapshot::new(UnitQueueState::Processing, 3)),
    )
    .await;
    for (lang, expected_phrase) in [
        (
            "en",
            "Previously queued and processing generation units keep their assigned attempt limit; only units queued after publication use the new limit",
        ),
        (
            "fa",
            "واحدهای تولید در صف پیشین و در حال پردازش سقف تلاش تخصیص‌یافته خود را حفظ می‌کنند؛ فقط واحدهای صف‌بندی‌شده پس از انتشار از سقف جدید استفاده می‌کنند",
        ),
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/console/changes/{attempt_id}?lang={lang}"))
                    .header(
                        header::COOKIE,
                        format!("tankhor_dev_admin_session={cookie}"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(resp.into_body(), 128 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            html.contains(expected_phrase),
            "missing {expected_phrase} in {lang}"
        );
    }

    // 8. Account recovery window OI8 / P41 / AC-ADM-046 preservation (account_recovery_days 30 -> 21)
    let recovery_draft = create_setting_draft(
        &fixture,
        &cookie,
        SettingKey::AccountRecoveryDays,
        ChangeValue::Number(21),
    )
    .await;
    let recovery_id = recovery_draft.id().to_string();
    let recovery_rev = recovery_draft.head().unwrap().to_string();
    run_service_preview(
        &fixture,
        &cookie,
        &recovery_id,
        &recovery_rev,
        PreviewEvidence::Recovery(PendingRecoverySnapshot::new(now + Duration::days(30))),
    )
    .await;
    for (lang, expected_phrase) in [
        (
            "en",
            "Accounts already in pending deletion keep their previously shown recovery end date D; only new deletion requests after publication use the updated recovery window",
        ),
        (
            "fa",
            "حساب‌های در انتظار حذف تاریخ پایان بازیابی نمایش‌داده‌شده پیشین (D) خود را حفظ می‌کنند؛ فقط درخواست‌های حذف جدید پس از انتشار از بازه بازیابی جدید استفاده می‌کنند",
        ),
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/console/changes/{recovery_id}?lang={lang}"))
                    .header(
                        header::COOKIE,
                        format!("tankhor_dev_admin_session={cookie}"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(resp.into_body(), 128 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            html.contains(expected_phrase),
            "missing {expected_phrase} in {lang}"
        );
    }
}
