//! Test-owned bootstrap resources, independent of shared endpoints and fallback launchers.

use std::{
    fmt::Write, fs, io::Read, os::unix::fs::PermissionsExt, path::PathBuf, process::Command,
};

use testkit::fixtures::{ownership::FixtureScope, postgres::PostgresFixture};

pub struct Bootstrap {
    pub directory: PathBuf,
    pub fixture: PostgresFixture,
}

impl Bootstrap {
    pub async fn start() -> Self {
        let scope_id = format!(
            "roles-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let directory = std::env::temp_dir().join(&scope_id);
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["admin", "migration", "runtime", "preparation"] {
            let mut entropy = [0_u8; 32];
            fs::File::open("/dev/urandom")
                .unwrap()
                .read_exact(&mut entropy)
                .unwrap();
            let password = entropy.iter().fold(String::new(), |mut encoded, byte| {
                write!(encoded, "{byte:02x}").unwrap();
                encoded
            });
            let path = directory.join(name);
            fs::write(&path, password).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let scope = FixtureScope::new("tankhor-bootstrap", &scope_id).unwrap();
        let fixture =
            PostgresFixture::start_or_reuse(&scope, "fixture_admin", &directory.join("admin"))
                .await
                .unwrap();
        Self { directory, fixture }
    }
}

impl Drop for Bootstrap {
    fn drop(&mut self) {
        let removed = Command::new("docker")
            .args(["rm", "-f", "-v", self.fixture.id()])
            .output();
        if !removed.is_ok_and(|output| output.status.success()) {
            eprintln!("bootstrap cleanup failed for test-owned container");
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}
