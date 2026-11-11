//! Current console authority is a read-only query, never a reusable access grant.
use crate::{
    domain::{admin_state::SecurityEpoch, secrets::SecretBytes},
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        admin_repository::AdminRepository,
        session_store::SessionStore,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ids::{AdminId, SessionId},
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleActor {
    pub admin: AdminId,
    pub session: SessionId,
    pub epoch: SecurityEpoch,
}

pub struct VerifyConsoleAccess<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
}

impl VerifyConsoleAccess<'_> {
    /// # Errors
    /// Refuses unknown cookies, incomplete factors, expired/revoked sessions and
    /// changed administrator authority. Mutations must independently reauthorize.
    pub async fn execute(&self, cookie: SecretBytes) -> Result<ConsoleActor, DomainError> {
        let digest = self
            .authenticator
            .token_digest(TokenPurpose::Session, cookie)
            .await?;
        let mut tx = self.uow.begin(TransactionMode::ReadOnly).await?;
        let result = async {
            let denied = || DomainError::unauthorised(ErrorCode::AuthenticationRequired);
            let session = self
                .sessions
                .lookup(&mut tx, digest)
                .await?
                .ok_or_else(denied)?;
            let admin = self
                .admins
                .get(&mut tx, session.admin_id())
                .await?
                .ok_or_else(denied)?;
            session.check_access(&admin, self.clock.now().await?)?;
            Ok(ConsoleActor {
                admin: admin.id(),
                session: session.id(),
                epoch: admin.epoch(),
            })
        }
        .await;
        match result {
            Ok(actor) => {
                self.uow.commit(tx).await?;
                Ok(actor)
            }
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}
