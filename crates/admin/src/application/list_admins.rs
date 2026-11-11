//! Read-only current-actor authorization followed by one bounded safe keyset page.
use crate::{
    domain::{
        admin_state::{EnrollmentPurpose, EnrollmentStatus, Permissions, SecurityEpoch},
        auth_state::BindingDigest,
    },
    ports::{
        admin_repository::{AdminListQuery, AdminRepository},
        session_store::SessionStore,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::AdminId,
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct ListAdminsInput {
    pub actor: BindingDigest,
    pub cursor: Option<String>,
    pub limit: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollmentSummary {
    pub purpose: EnrollmentPurpose,
    pub authorizer: AdminId,
    pub status: EnrollmentStatus,
    pub expires_at: OffsetDateTime,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminSummary {
    pub id: AdminId,
    pub epoch: SecurityEpoch,
    pub permissions: Option<Permissions>,
    pub enrollment: Option<EnrollmentSummary>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminList {
    pub entries: Vec<AdminSummary>,
    pub next: Option<String>,
}
pub struct ListAdmins<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub clock: &'a dyn Clock,
}
impl ListAdmins<'_> {
    /// # Errors
    /// Refuses malformed cursors/zero bounds and sessions without current full access.
    pub async fn execute(&self, input: ListAdminsInput) -> Result<AdminList, DomainError> {
        let after = input
            .cursor
            .map(|cursor| cursor.parse::<AdminId>())
            .transpose()
            .map_err(|_| DomainError::validation(ErrorCode::InvalidValue))?;
        let query = AdminListQuery::new(after, input.limit)?;
        let mut tx = self.uow.begin(TransactionMode::ReadOnly).await?;
        let result = async {
            let session = self
                .sessions
                .lookup(&mut tx, input.actor)
                .await?
                .ok_or_else(denied)?;
            let admin = self
                .admins
                .get(&mut tx, session.admin_id())
                .await?
                .ok_or_else(denied)?;
            session.check_access(&admin, self.clock.now().await?)?;
            let page = self.admins.list(&mut tx, query).await?;
            Ok(AdminList {
                entries: page
                    .entries
                    .into_iter()
                    .map(|admin| AdminSummary {
                        id: admin.id(),
                        epoch: admin.epoch(),
                        permissions: admin.permissions(),
                        enrollment: admin.enrollment().map(|authorization| EnrollmentSummary {
                            purpose: authorization.purpose(),
                            authorizer: authorization.authorizer(),
                            status: authorization.status(),
                            expires_at: authorization.expires_at(),
                        }),
                    })
                    .collect(),
                next: page.next.map(|id| id.to_string()),
            })
        }
        .await;
        match result {
            Ok(page) => {
                self.uow.commit(tx).await?;
                Ok(page)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}
fn denied() -> DomainError {
    DomainError::unauthorised(ErrorCode::AuthenticationRequired)
}
