#[path = "support/settings.rs"]
mod settings;

use admin::{
    application::{
        save_draft::{SaveDraft, SaveDraftInput},
        seed_foundation::SeedOutcome,
    },
    domain::{
        audit::{AuditAction, ReasonCode},
        change_revision::{ChangeSubject, ChangeValue},
        settings::{SettingKey, SettingValue},
    },
    ports::{
        audit_repository::{AuditFilter, AuditQuery, AuditRepository},
        change_repository::ChangeRepository,
        flag_repository::FlagRepository,
        health_probe::{HealthProbe, ProbeReadiness},
        settings_repository::SettingsRepository,
    },
};
use kernel::{
    error::{ErrorCategory, ErrorCode},
    ports::unit_of_work::{TransactionMode, UnitOfWork},
};
use settings::{
    CoordinatedIds, RealSettingsFixture, creator_admin, creator_session, first_editor_admin,
    first_editor_session, second_editor_admin, second_editor_session,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use testkit::fixtures::database::FixtureDatabase;
use tokio::sync::Barrier;

#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]
async fn registry_seed_and_draft_commit_with_audit_or_not_at_all(
    admin_pool_options: PgPoolOptions,
    admin_options: PgConnectOptions,
) {
    let database = FixtureDatabase::connect(admin_pool_options, admin_options)
        .await
        .expect("connect migrated fixture database");
    database
        .verify_migrated_and_role_separation()
        .await
        .expect("restricted runtime role and migrations must be ready");
    let fixture = RealSettingsFixture::connect(database).await;
    fixture.seed_authenticated_admins().await;

    assert_eq!(
        fixture.services.probe.check().await.unwrap(),
        ProbeReadiness::Unseeded
    );

    // 1. Audit failure during SeedFoundation rolls back settings, flags, and audit atomically.
    fixture.install_audit_rejection().await;
    assert!(fixture.seed_foundation(100).await.is_err());
    assert_eq!(
        fixture.services.probe.check().await.unwrap(),
        ProbeReadiness::Unseeded
    );
    assert_eq!(fixture.total_audit_count().await, 0);
    {
        let mut tx = fixture
            .services
            .uow
            .begin(TransactionMode::ReadOnly)
            .await
            .unwrap();
        assert!(
            fixture
                .services
                .settings
                .list(&mut tx)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            fixture
                .services
                .flags
                .list(&mut tx)
                .await
                .unwrap()
                .is_empty()
        );
        fixture.services.uow.commit(tx).await.unwrap();
    }
    fixture.remove_audit_rejection().await;

    // 2. SeedFoundation commits 10 settings, 6 OFF flags, and 1 FoundationSeeded audit atomically.
    assert_eq!(
        fixture.seed_foundation(101).await.unwrap(),
        SeedOutcome::Seeded
    );
    assert_eq!(
        fixture.services.probe.check().await.unwrap(),
        ProbeReadiness::Ready
    );
    assert_eq!(
        fixture.audit_count_for(AuditAction::FoundationSeeded).await,
        1
    );

    // Advance live UploadMaxMb to 25 and confirm SeedFoundation rerun is idempotent and preserves it.
    fixture
        .publish_setting_value(SettingKey::UploadMaxMb, SettingValue::Number(25), 200)
        .await;
    assert_eq!(
        fixture.seed_foundation(102).await.unwrap(),
        SeedOutcome::AlreadyApplied
    );
    assert_eq!(
        fixture.audit_count_for(AuditAction::FoundationSeeded).await,
        1
    );
    {
        let mut tx = fixture
            .services
            .uow
            .begin(TransactionMode::ReadOnly)
            .await
            .unwrap();
        let settings_list = fixture.services.settings.list(&mut tx).await.unwrap();
        let flags_list = fixture.services.flags.list(&mut tx).await.unwrap();
        assert_eq!(settings_list.len(), 10);
        assert_eq!(flags_list.len(), 6);
        assert!(flags_list.iter().all(|flag| !flag.version().value()));
        let current_upload = fixture
            .services
            .settings
            .get(&mut tx, SettingKey::UploadMaxMb)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current_upload.version().value(), &SettingValue::Number(25));
        fixture.services.uow.commit(tx).await.unwrap();
    }

    // 3. Audit failure during initial SaveDraft rolls back change_drafts and change_revisions atomically.
    fixture.install_audit_rejection().await;
    assert!(
        fixture
            .save_draft(creator_session(), None, None, ChangeValue::Number(20))
            .await
            .is_err()
    );
    assert_eq!(fixture.change_draft_count().await, 0);
    assert_eq!(fixture.change_revision_count().await, 0);
    assert_eq!(fixture.audit_count_for(AuditAction::DraftSaved).await, 0);
    fixture.remove_audit_rejection().await;

    // 4. Initial SaveDraft commits draft revision and audit without mutating live setting pointer.
    let created = fixture
        .save_draft(creator_session(), None, None, ChangeValue::Number(20))
        .await
        .expect("initial draft save succeeds");
    let change_id = created.id();
    let first_head = created.head().expect("draft has head revision");
    assert_eq!(created.creator(), creator_admin());
    assert_eq!(created.revisions().len(), 1);
    assert_eq!(created.revisions()[0].created_by(), creator_admin());
    assert_eq!(created.revisions()[0].value(), ChangeValue::Number(20));
    assert_eq!(fixture.change_draft_count().await, 1);
    assert_eq!(fixture.change_revision_count().await, 1);
    assert_eq!(fixture.audit_count_for(AuditAction::DraftSaved).await, 1);
    {
        let mut tx = fixture
            .services
            .uow
            .begin(TransactionMode::ReadOnly)
            .await
            .unwrap();
        let live = fixture
            .services
            .settings
            .get(&mut tx, SettingKey::UploadMaxMb)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(live.version().value(), &SettingValue::Number(25));
        fixture.services.uow.commit(tx).await.unwrap();
    }

    // 5. Audit failure during revision append rolls back head pointer and appended revision atomically.
    fixture.install_audit_rejection().await;
    assert!(
        fixture
            .save_draft(
                first_editor_session(),
                Some(change_id),
                Some(first_head),
                ChangeValue::Number(18),
            )
            .await
            .is_err()
    );
    assert_eq!(fixture.change_draft_count().await, 1);
    assert_eq!(fixture.change_revision_count().await, 1);
    assert_eq!(fixture.audit_count_for(AuditAction::DraftSaved).await, 1);
    {
        let mut tx = fixture
            .services
            .uow
            .begin(TransactionMode::ReadOnly)
            .await
            .unwrap();
        let stored = fixture
            .services
            .changes
            .draft(&mut tx, change_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.head(), Some(first_head));
        assert_eq!(stored.revisions().len(), 1);
        assert_eq!(stored.creator(), creator_admin());
        fixture.services.uow.commit(tx).await.unwrap();
    }
    fixture.remove_audit_rejection().await;

    // 6. Concurrent stale draft saves: two editors race with the same expected_head;
    //    exactly one winner commits and records an audit event, while the loser is rejected.
    let barrier = Barrier::new(2);
    let first_ids = CoordinatedIds::new(&fixture.services.ids, &barrier);
    let second_ids = CoordinatedIds::new(&fixture.services.ids, &barrier);
    let first_save = SaveDraft {
        uow: &fixture.services.uow,
        sessions: &fixture.services.sessions,
        admins: &fixture.services.admins,
        settings: &fixture.services.settings,
        flags: &fixture.services.flags,
        changes: &fixture.services.changes,
        audit: &fixture.services.audit,
        clock: &fixture.services.clock,
        ids: &first_ids,
    };
    let second_save = SaveDraft {
        uow: &fixture.services.uow,
        sessions: &fixture.services.sessions,
        admins: &fixture.services.admins,
        settings: &fixture.services.settings,
        flags: &fixture.services.flags,
        changes: &fixture.services.changes,
        audit: &fixture.services.audit,
        clock: &fixture.services.clock,
        ids: &second_ids,
    };
    let (first_res, second_res) = tokio::join!(
        first_save.execute(SaveDraftInput {
            actor: first_editor_session(),
            change: Some(change_id),
            subject: ChangeSubject::Setting(SettingKey::UploadMaxMb),
            value: ChangeValue::Number(16),
            reason: ReasonCode::DraftChange,
            expected_head: Some(first_head),
            schedule: None,
        }),
        second_save.execute(SaveDraftInput {
            actor: second_editor_session(),
            change: Some(change_id),
            subject: ChangeSubject::Setting(SettingKey::UploadMaxMb),
            value: ChangeValue::Number(19),
            reason: ReasonCode::DraftChange,
            expected_head: Some(first_head),
            schedule: None,
        }),
    );

    assert_eq!(
        usize::from(first_res.is_ok()) + usize::from(second_res.is_ok()),
        1,
        "exactly one concurrent SaveDraft must win: first={first_res:?}, second={second_res:?}"
    );
    let (winner_draft, winner_admin, winner_value, loser_err) = match (first_res, second_res) {
        (Ok(draft), Err(err)) => (draft, first_editor_admin(), ChangeValue::Number(16), err),
        (Err(err), Ok(draft)) => (draft, second_editor_admin(), ChangeValue::Number(19), err),
        _ => unreachable!("exactly one winner verified above"),
    };
    assert_eq!(loser_err.category(), ErrorCategory::Conflict);
    assert_eq!(loser_err.code(), ErrorCode::StaleRevision);

    assert_eq!(fixture.change_draft_count().await, 1);
    assert_eq!(fixture.change_revision_count().await, 2);
    assert_eq!(fixture.audit_count_for(AuditAction::DraftSaved).await, 2);

    {
        let mut tx = fixture
            .services
            .uow
            .begin(TransactionMode::ReadOnly)
            .await
            .unwrap();
        let stored = fixture
            .services
            .changes
            .draft(&mut tx, change_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored, winner_draft);
        assert_eq!(stored.creator(), creator_admin());
        assert_eq!(stored.revisions().len(), 2);
        assert_eq!(stored.revisions()[0].created_by(), creator_admin());
        assert_eq!(stored.revisions()[0].value(), ChangeValue::Number(20));
        assert_eq!(stored.revisions()[1].created_by(), winner_admin);
        assert_eq!(stored.revisions()[1].value(), winner_value);

        let page = fixture
            .services
            .audit
            .search(
                &mut tx,
                AuditQuery::new(
                    AuditFilter::new(
                        None,
                        Some(AuditAction::DraftSaved),
                        Some(admin::domain::audit::Subject::Change(change_id)),
                        None,
                        None,
                    )
                    .unwrap(),
                    None,
                    10,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(page.entries.len(), 2);
        assert_eq!(
            page.entries[0].actor(),
            admin::domain::audit::Actor::Admin(creator_admin())
        );
        assert_eq!(
            page.entries[1].actor(),
            admin::domain::audit::Actor::Admin(winner_admin)
        );
        fixture.services.uow.commit(tx).await.unwrap();
    }

    fixture.close().await;
}
