//! T015: FR-024/FR-CON-37 cluster roles, idempotency and restricted runtime access.

#[path = "support/bootstrap.rs"]
mod bootstrap;

use sqlx::{Connection, Executor, PgConnection};
use testkit::fixtures::roles::provision_roles;

#[tokio::test]
async fn fixture_roles_are_idempotent_and_runtime_is_not_owner() {
    let owned = bootstrap::Bootstrap::start().await;
    let options = owned.fixture.connect_options();
    let (first, second) = tokio::join!(
        provision_roles(&options, &owned.directory),
        provision_roles(&options, &owned.directory),
    );
    let roles = first.unwrap();
    second.unwrap();
    provision_roles(&options, &owned.directory).await.unwrap();
    let mut admin = PgConnection::connect_with(&options).await.unwrap();
    let privileges: (bool, bool, bool, bool, bool) = sqlx::query_as(
        "SELECT rolsuper, rolcreatedb, rolcreaterole, rolreplication, rolbypassrls \
         FROM pg_roles WHERE rolname = 'tankhor_fixture_runtime'",
    )
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert_eq!(privileges, (false, false, false, false, false));
    let membership: bool = sqlx::query_scalar(
        "SELECT pg_has_role('tankhor_fixture_runtime', 'tankhor_fixture_migration', 'MEMBER') \
         OR pg_has_role('tankhor_fixture_runtime', 'tankhor_fixture_preparation', 'MEMBER') \
         OR pg_has_role('tankhor_fixture_runtime', 'fixture_admin', 'MEMBER')",
    )
    .fetch_one(&mut admin)
    .await
    .unwrap();
    assert!(!membership);
    admin
        .execute("CREATE DATABASE fixture_role_check OWNER tankhor_fixture_migration")
        .await
        .unwrap();
    let migration = roles.migration(&options).database("fixture_role_check");
    let mut migration_connection = PgConnection::connect_with(&migration).await.unwrap();
    migration_connection
        .execute("CREATE TABLE role_probe (id integer PRIMARY KEY)")
        .await
        .unwrap();
    migration_connection
        .execute("GRANT SELECT, INSERT, UPDATE, DELETE ON role_probe TO tankhor_fixture_runtime")
        .await
        .unwrap();
    let mut runtime =
        PgConnection::connect_with(&roles.runtime(&options).database("fixture_role_check"))
            .await
            .unwrap();
    runtime
        .execute("INSERT INTO role_probe VALUES (1)")
        .await
        .unwrap();
    for forbidden in [
        "ALTER TABLE role_probe ADD COLUMN forbidden integer",
        "DROP TABLE role_probe",
        "CREATE ROLE fixture_escalation",
        "CREATE DATABASE fixture_escalation",
        "SET ROLE tankhor_fixture_migration",
        "SET ROLE tankhor_fixture_preparation",
        "SET ROLE fixture_admin",
    ] {
        let error = runtime
            .execute(forbidden)
            .await
            .expect_err("runtime must remain restricted");
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("42501")
        );
    }
    let table_owner: String =
        sqlx::query_scalar("SELECT tableowner FROM pg_tables WHERE tablename = 'role_probe'")
            .fetch_one(&mut runtime)
            .await
            .unwrap();
    assert_eq!(table_owner, "tankhor_fixture_migration");
    let mut preparation = PgConnection::connect_with(&roles.preparation(&options))
        .await
        .unwrap();
    let current: String = sqlx::query_scalar("SELECT current_user")
        .fetch_one(&mut preparation)
        .await
        .unwrap();
    assert_eq!(current, "tankhor_fixture_preparation");
    runtime.close().await.unwrap();
    migration_connection.close().await.unwrap();
    preparation.close().await.unwrap();
    admin.close().await.unwrap();
}
