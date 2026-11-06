//! T021 setup smoke: tested owned PostgreSQL lifecycle through nextest shared setup.

use std::{collections::BTreeMap, os::unix::fs::PermissionsExt, process::Command};

use sqlx::{Connection, PgConnection};
use testkit::fixtures::ownership::{Endpoint, FixtureScope};

#[tokio::test]
async fn shared_fixture_exports_owned_ready_postgres() {
    let url = std::env::var("DATABASE_URL").expect("shared setup must export an endpoint");
    assert!(
        !url.contains("fixture_admin:"),
        "URL must not contain a password"
    );
    let passfile = std::env::var("PGPASSFILE").unwrap();
    assert_eq!(
        std::fs::metadata(passfile).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let scope = FixtureScope::new(
        &std::env::var("FIXTURE_PROJECT").unwrap(),
        &std::env::var("FIXTURE_RUN").unwrap(),
    )
    .unwrap();
    let id = std::env::var("FIXTURE_CONTAINER").unwrap();
    let mut labels = BTreeMap::new();
    for key in scope.postgres_labels().keys() {
        let value = Command::new("docker")
            .args([
                "inspect",
                "--format",
                &format!("{{{{index .Config.Labels \"{key}\"}}}}"),
                &id,
            ])
            .output()
            .unwrap();
        assert!(value.status.success());
        labels.insert(
            key.clone(),
            String::from_utf8(value.stdout).unwrap().trim().to_owned(),
        );
    }
    let options: sqlx::postgres::PgConnectOptions = url.parse().unwrap();
    let endpoint = Endpoint::new(options.get_host(), options.get_port()).unwrap();
    scope.validate(&labels, endpoint, endpoint).unwrap();
    let mut connection = PgConnection::connect(&url).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&mut connection)
            .await
            .unwrap(),
        1
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_roles WHERE rolname IN \
        ('tankhor_fixture_migration', 'tankhor_fixture_runtime', 'tankhor_fixture_preparation')",
    )
    .fetch_one(&mut connection)
    .await
    .unwrap();
    assert_eq!(count, 3);
    connection.close().await.unwrap();
}
