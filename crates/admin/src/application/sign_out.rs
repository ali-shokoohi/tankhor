//! Revoke only the presented session after rechecking current locked authority.
use crate::{
    domain::secrets::SecretBytes,
    ports::{
        admin_authenticator::{AdminAuthenticator, TokenPurpose},
        admin_repository::AdminRepository,
        session_store::SessionStore,
    },
};
use kernel::{
    error::{DomainError, ErrorCode},
    ports::{
        clock::Clock,
        unit_of_work::{TransactionMode, UnitOfWork},
    },
};

pub struct SignOut<'a> {
    pub uow: &'a dyn UnitOfWork,
    pub admins: &'a dyn AdminRepository,
    pub sessions: &'a dyn SessionStore,
    pub authenticator: &'a dyn AdminAuthenticator,
    pub clock: &'a dyn Clock,
}
impl SignOut<'_> {
    /// # Errors
    /// Refuses unknown/expired/stale authority and rolls back failed revocation.
    /// Repeating sign-out never restores a revoked session.
    pub async fn execute(&self, cookie: SecretBytes) -> Result<(), DomainError> {
        let digest = self
            .authenticator
            .token_digest(TokenPurpose::Session, cookie)
            .await?;
        let mut tx = self.uow.begin(TransactionMode::ReadWrite).await?;
        let result = async {
            let denied = || DomainError::unauthorised(ErrorCode::AuthenticationRequired);
            let session = self
                .sessions
                .lookup(&mut tx, digest)
                .await?
                .ok_or_else(denied)?;
            let locked = self.admins.lock(&mut tx, vec![session.admin_id()]).await?;
            let admin = locked
                .iter()
                .find(|admin| admin.id() == session.admin_id())
                .ok_or_else(denied)?;
            let now = self.clock.now().await?;
            session.check_access(admin, now)?;
            self.sessions.revoke_one(&mut tx, session.id(), now).await
        }
        .await;
        match result {
            Ok(()) => self.uow.commit(tx).await,
            Err(error) => {
                self.uow.rollback(tx).await?;
                Err(error)
            }
        }
    }
}
