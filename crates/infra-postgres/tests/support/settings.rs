#![allow(dead_code)]

use std::str::FromStr;

use admin::{
    application::{
        save_draft::{SaveDraft, SaveDraftInput},
        seed_foundation::{SeedFoundation, SeedFoundationInput, SeedOutcome},
    },
    domain::{
        admin_state::{AdminState, Eligibility, SecurityEpoch},
        audit::{AuditAction, Factor, ReasonCode},
        auth_state::BindingDigest,
        change_revision::{ChangeDraft, ChangeSubject, ChangeValue},
        session_state::{SessionInput, SessionState},
        settings::{SettingKey, SettingValue},
    },
    ports::{
        admin_repository::{AdminRepository, IdentityDigest},
        session_store::{SessionRecord, SessionStore},
        settings_repository::{SettingVersion, SettingsRepository},
    },
};
use infra_postgres::{
    admin_repository::PostgresAdminRepository, audit_repository::PostgresAuditRepository,
    change_repository::PostgresChangeRepository, flag_repository::PostgresFlagRepository,
    health_probe::PostgresHealthProbe, session_store::PostgresSessionStore,
    settings_repository::PostgresSettingsRepository, unit_of_work::PostgresUnitOfWork,
};
use kernel::{
    error::DomainError,
    ids::{
        AdminId, ApprovalId, AuditId, ChallengeId, ChangeId, CredentialId, PreviewId,
        PublicationId, RevisionId, SessionId, VersionId,
    },
    ports::{
        id_generator::IdGenerator,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};
use testkit::{
    fakes::{clock::FixedClock, id_generator::SequenceIdGenerator},
    fixtures::database::FixtureDatabase,
};
use time::{Duration, OffsetDateTime, macros::datetime};

pub fn now() -> OffsetDateTime {
    datetime!(2026-01-01 00:00 UTC)
}

pub fn id<T: FromStr>(n: u16) -> T {
    format!("019a53a0-6200-7000-8000-{n:012x}")
        .parse()
        .ok()
        .expect("valid v7 fixture id")
}

pub fn system_admin() -> AdminId {
    id(1)
}

pub fn creator_admin() -> AdminId {
    id(2)
}

pub fn first_editor_admin() -> AdminId {
    id(3)
}

pub fn second_editor_admin() -> AdminId {
    id(4)
}

pub fn creator_session() -> BindingDigest {
    BindingDigest::new([0x21; 32])
}

pub fn first_editor_session() -> BindingDigest {
    BindingDigest::new([0x22; 32])
}

pub fn second_editor_session() -> BindingDigest {
    BindingDigest::new([0x23; 32])
}

pub struct RealSettings {
    pub uow: PostgresUnitOfWork,
    pub admins: PostgresAdminRepository,
    pub sessions: PostgresSessionStore,
    pub settings: PostgresSettingsRepository,
    pub flags: PostgresFlagRepository,
    pub changes: PostgresChangeRepository,
    pub audit: PostgresAuditRepository,
    pub probe: PostgresHealthProbe,
    pub clock: FixedClock,
    pub ids: SequenceIdGenerator,
}

pub struct RealSettingsFixture {
    pub database: FixtureDatabase,
    pub services: RealSettings,
}

impl RealSettingsFixture {
    pub async fn connect(database: FixtureDatabase) -> Self {
        let pool = database
            .connect_runtime_pool(8)
            .await
            .expect("connect restricted runtime settings pool");
        let uow = PostgresUnitOfWork::new(pool.clone());
        let services = RealSettings {
            admins: PostgresAdminRepository::new(uow.clone()),
            sessions: PostgresSessionStore::new(uow.clone()),
            settings: PostgresSettingsRepository::new(uow.clone()),
            flags: PostgresFlagRepository::new(uow.clone()),
            changes: PostgresChangeRepository::new(uow.clone()),
            audit: PostgresAuditRepository::new(uow.clone()),
            probe: PostgresHealthProbe::new(pool),
            clock: FixedClock::new(now()),
            ids: SequenceIdGenerator::new(1_000),
            uow,
        };
        Self { database, services }
    }

    pub async fn seed_authenticated_admins(&self) {
        let actors = [
            (creator_admin(), creator_session(), 1_u16, 0x41_u8),
            (first_editor_admin(), first_editor_session(), 2_u16, 0x42_u8),
            (
                second_editor_admin(),
                second_editor_session(),
                3_u16,
                0x43_u8,
            ),
        ];
        let mut tx = self
            .services
            .uow
            .begin(TransactionMode::ReadWrite)
            .await
            .expect("begin admin seed transaction");
        for (admin_id, digest, session_seq, phone_tag) in actors {
            let state = AdminState::new(admin_id, Eligibility::Active, SecurityEpoch::new(0));
            self.services
                .admins
                .invite(&mut tx, state.clone(), IdentityDigest::new([phone_tag; 32]))
                .await
                .expect("invite active admin");
            self.services
                .sessions
                .insert(
                    &mut tx,
                    SessionRecord {
                        digest,
                        state: SessionState::try_from(SessionInput {
                            id: id::<SessionId>(500 + session_seq),
                            admin_id,
                            epoch: state.epoch(),
                            primary_verified: true,
                            factor: Some(Factor::Passkey),
                            issued_at: now(),
                            expires_at: now() + Duration::minutes(30),
                            idle_timeout: Duration::minutes(15),
                        })
                        .expect("valid session state"),
                    },
                )
                .await
                .expect("insert active session");
        }
        self.services
            .uow
            .commit(tx)
            .await
            .expect("commit admin seed transaction");
    }

    pub async fn seed_foundation(&self, operation_seq: u16) -> Result<SeedOutcome, DomainError> {
        SeedFoundation {
            uow: &self.services.uow,
            settings: &self.services.settings,
            flags: &self.services.flags,
            audit: &self.services.audit,
            clock: &self.services.clock,
            ids: &self.services.ids,
        }
        .execute(SeedFoundationInput {
            operation: id::<AuditId>(operation_seq),
            system: system_admin(),
        })
        .await
    }

    pub async fn save_draft(
        &self,
        actor: BindingDigest,
        change: Option<ChangeId>,
        expected_head: Option<RevisionId>,
        value: ChangeValue,
    ) -> Result<ChangeDraft, DomainError> {
        SaveDraft {
            uow: &self.services.uow,
            sessions: &self.services.sessions,
            admins: &self.services.admins,
            settings: &self.services.settings,
            flags: &self.services.flags,
            changes: &self.services.changes,
            audit: &self.services.audit,
            clock: &self.services.clock,
            ids: &self.services.ids,
        }
        .execute(SaveDraftInput {
            actor,
            change,
            subject: ChangeSubject::Setting(SettingKey::UploadMaxMb),
            value,
            reason: ReasonCode::DraftChange,
            expected_head,
            schedule: None,
        })
        .await
    }

    pub async fn publish_setting_value(&self, key: SettingKey, value: SettingValue, seq: u16) {
        let mut tx = self
            .services
            .uow
            .begin(TransactionMode::ReadWrite)
            .await
            .expect("begin publish transaction");
        let baseline = self
            .services
            .settings
            .get(&mut tx, key)
            .await
            .expect("load current setting")
            .expect("setting exists")
            .version()
            .version();
        let next = SettingVersion::new(id::<VersionId>(seq), key, value, creator_admin(), now())
            .expect("valid setting version");
        self.services
            .settings
            .append_version(&mut tx, next.clone())
            .await
            .expect("append setting version");
        self.services
            .settings
            .set_live(&mut tx, key, Some(baseline), next.version())
            .await
            .expect("advance live version");
        self.services
            .uow
            .commit(tx)
            .await
            .expect("commit publish transaction");
    }

    pub async fn install_audit_rejection(&self) {
        sqlx::raw_sql(
            "CREATE OR REPLACE FUNCTION reject_settings_audit()
             RETURNS TRIGGER LANGUAGE plpgsql SECURITY DEFINER AS $$
             BEGIN
                 RAISE EXCEPTION 'injected settings audit failure';
             END;
             $$;
             CREATE TRIGGER reject_settings_audit
             BEFORE INSERT ON audit_log
             FOR EACH ROW EXECUTE FUNCTION reject_settings_audit();",
        )
        .execute(self.database.admin_pool())
        .await
        .expect("install audit rejection trigger");
    }

    pub async fn remove_audit_rejection(&self) {
        sqlx::raw_sql("DROP TRIGGER IF EXISTS reject_settings_audit ON audit_log")
            .execute(self.database.admin_pool())
            .await
            .expect("remove audit rejection trigger");
    }

    pub async fn audit_count_for(&self, action: AuditAction) -> i64 {
        let name = match action {
            AuditAction::FoundationSeeded => "foundation_seeded",
            AuditAction::DraftSaved => "draft_saved",
            AuditAction::Previewed => "previewed",
            _ => "other",
        };
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = $1")
            .bind(name)
            .fetch_one(self.services.uow.pool())
            .await
            .expect("count audit rows")
    }

    pub async fn total_audit_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(self.services.uow.pool())
            .await
            .expect("count total audit rows")
    }

    pub async fn change_draft_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM change_drafts")
            .fetch_one(self.services.uow.pool())
            .await
            .expect("count change drafts")
    }

    pub async fn change_revision_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM change_revisions")
            .fetch_one(self.services.uow.pool())
            .await
            .expect("count change revisions")
    }

    pub async fn close(self) {
        self.services.uow.pool().close().await;
        self.database.close().await;
    }
}

pub struct CoordinatedIds<'a> {
    inner: &'a SequenceIdGenerator,
    revision_barrier: &'a tokio::sync::Barrier,
}

impl<'a> CoordinatedIds<'a> {
    pub const fn new(
        inner: &'a SequenceIdGenerator,
        revision_barrier: &'a tokio::sync::Barrier,
    ) -> Self {
        Self {
            inner,
            revision_barrier,
        }
    }
}

#[async_trait::async_trait]
impl IdGenerator for CoordinatedIds<'_> {
    async fn next_admin_id(&self) -> Result<AdminId, DomainError> {
        self.inner.next_admin_id().await
    }

    async fn next_credential_id(&self) -> Result<CredentialId, DomainError> {
        self.inner.next_credential_id().await
    }

    async fn next_session_id(&self) -> Result<SessionId, DomainError> {
        self.inner.next_session_id().await
    }

    async fn next_challenge_id(&self) -> Result<ChallengeId, DomainError> {
        self.inner.next_challenge_id().await
    }

    async fn next_change_id(&self) -> Result<ChangeId, DomainError> {
        self.inner.next_change_id().await
    }

    async fn next_revision_id(&self) -> Result<RevisionId, DomainError> {
        self.revision_barrier.wait().await;
        self.inner.next_revision_id().await
    }

    async fn next_preview_id(&self) -> Result<PreviewId, DomainError> {
        self.inner.next_preview_id().await
    }

    async fn next_approval_id(&self) -> Result<ApprovalId, DomainError> {
        self.inner.next_approval_id().await
    }

    async fn next_publication_id(&self) -> Result<PublicationId, DomainError> {
        self.inner.next_publication_id().await
    }

    async fn next_version_id(&self) -> Result<VersionId, DomainError> {
        self.inner.next_version_id().await
    }

    async fn next_audit_id(&self) -> Result<AuditId, DomainError> {
        self.inner.next_audit_id().await
    }
}
