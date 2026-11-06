//! T013: FR-024/FR-CON-37, test-owned startup, reuse and SQL readiness.

use std::{
    fmt::Write, fs, io::Read, os::unix::fs::PermissionsExt, path::PathBuf, process::Command,
};

use sqlx::{Connection, PgConnection};
use testkit::fixtures::{ownership::FixtureScope, postgres::PostgresFixture};

struct Cleanup {
    directory: PathBuf,
    container: Option<String>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(id) = &self.container {
            let _ = Command::new("docker").args(["unpause", id]).output();
            let _ = Command::new("docker").args(["rm", "-f", "-v", id]).output();
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn docker(arguments: &[&str]) {
    let result = Command::new("docker").args(arguments).output().unwrap();
    assert!(result.status.success(), "Docker lifecycle operation failed");
}

#[tokio::test]
async fn postgres_fixture_startup_is_idempotent_and_waits_for_readiness() {
    let mut entropy = [0_u8; 32];
    fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut entropy)
        .unwrap();
    let password = entropy.iter().fold(String::new(), |mut encoded, byte| {
        write!(encoded, "{byte:02x}").unwrap();
        encoded
    });
    let scope_id = format!(
        "bootstrap-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let directory = std::env::temp_dir().join(&scope_id);
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    let secret = directory.join("postgres-password");
    fs::write(&secret, &password).unwrap();
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
    let mut cleanup = Cleanup {
        directory,
        container: None,
    };
    let scope = FixtureScope::new("tankhor-bootstrap", &scope_id).unwrap();
    let first = PostgresFixture::start_or_reuse(&scope, "fixture_admin", &secret)
        .await
        .unwrap();
    cleanup.container = Some(first.id().to_owned());
    let mut connection = PgConnection::connect_with(&first.connect_options())
        .await
        .unwrap();
    let version: String = sqlx::query_scalar("SHOW server_version")
        .fetch_one(&mut connection)
        .await
        .unwrap();
    assert!(version.starts_with("16."));
    connection.close().await.unwrap();

    let id = first.id().to_owned();
    let endpoint = first.endpoint();
    drop(first); // Shared services must outlive any helper/child guard.
    let reused = PostgresFixture::start_or_reuse(&scope, "fixture_admin", &secret)
        .await
        .unwrap();
    assert_eq!(reused.id(), id);
    assert_eq!(reused.endpoint(), endpoint);
    let inspect = Command::new("docker").args([
        "inspect", "--format", "{{range $p, $bindings := .NetworkSettings.Ports}}{{range $bindings}}{{.HostIp}}{{end}}{{end}}", &id,
    ]).output().unwrap();
    assert!(inspect.status.success());
    assert_eq!(
        String::from_utf8(inspect.stdout).unwrap().trim(),
        "127.0.0.1"
    );
    drop(reused);

    docker(&["pause", &id]);
    let pending = tokio::spawn(async move {
        PostgresFixture::start_or_reuse(&scope, "fixture_admin", &secret).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(
        !pending.is_finished(),
        "container existence is not SQL readiness"
    );
    docker(&["unpause", &id]);
    let ready = pending.await.unwrap().unwrap();
    assert_eq!(
        ready.id(),
        id,
        "readiness wait must not create a second server"
    );
    let mut connection = PgConnection::connect_with(&ready.connect_options())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&mut connection)
            .await
            .unwrap(),
        1
    );
    connection.close().await.unwrap();
}
