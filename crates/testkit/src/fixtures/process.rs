//! Child-only fixture endpoints and process-scoped fallback lifetime (T023).

use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{Read, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        process::ExitStatusExt,
    },
    path::{Path, PathBuf},
    process::ExitStatus,
};

use super::{ownership::FixtureScope, postgres::PostgresFixture};

/// Sanitized process/fixture refusal.
#[derive(Debug)]
pub struct ProcessError;
impl std::fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("fixture child launch refused")
    }
}
impl std::error::Error for ProcessError {}

struct Cleanup {
    directory: PathBuf,
    container: Option<String>,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(id) = &self.container {
            let _ = std::process::Command::new("docker")
                .args(["rm", "-f", "-v", id])
                .output();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// Launch the same test binary with validated child-only endpoints and retain resources
/// until it exits. Supplied endpoints are validated before the test body/SQLx bootstrap.
///
/// # Errors
/// Refuses foreign endpoints, fixture failures or inability to execute the exact child.
pub async fn run_child(binary: &Path, arguments: &[OsString]) -> Result<u8, ProcessError> {
    let identity = format!("fallback-{}", std::process::id());
    let directory = std::env::temp_dir().join(&identity);
    std::fs::create_dir(&directory).map_err(|_| ProcessError)?;
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| ProcessError)?;
    let mut cleanup = Cleanup {
        directory: directory.clone(),
        container: None,
    };
    let mut environment = BTreeMap::new();
    let credential_directory;
    if let Ok(url) = std::env::var("DATABASE_URL") {
        let options: sqlx::postgres::PgConnectOptions = url.parse().map_err(|_| ProcessError)?;
        if options.get_username() != "fixture_admin" || url.contains("fixture_admin:") {
            return Err(ProcessError);
        }
        super::ownership::Endpoint::new(options.get_host(), options.get_port())
            .map_err(|_| ProcessError)?;
        for name in ["FIXTURE_PROJECT", "FIXTURE_RUN", "FIXTURE_CONTAINER"] {
            environment.insert(
                name.to_owned(),
                std::env::var(name).map_err(|_| ProcessError)?,
            );
        }
        let passfile = PathBuf::from(std::env::var_os("PGPASSFILE").ok_or(ProcessError)?);
        credential_directory = passfile.parent().ok_or(ProcessError)?.to_owned();
        environment.insert("FIXTURE_HOST".to_owned(), options.get_host().to_owned());
        environment.insert("FIXTURE_PORT".to_owned(), options.get_port().to_string());
    } else {
        prepare_credentials(&directory)?;
        let scope = FixtureScope::new("tankhor-fallback", &identity).map_err(|_| ProcessError)?;
        let fixture =
            PostgresFixture::start_or_reuse(&scope, "fixture_admin", &directory.join("admin"))
                .await
                .map_err(|_| ProcessError)?;
        cleanup.container = Some(fixture.id().to_owned());
        let options = fixture.connect_options();
        environment.insert("FIXTURE_PROJECT".to_owned(), "tankhor-fallback".to_owned());
        environment.insert("FIXTURE_RUN".to_owned(), identity);
        environment.insert("FIXTURE_CONTAINER".to_owned(), fixture.id().to_owned());
        environment.insert("FIXTURE_HOST".to_owned(), options.get_host().to_owned());
        environment.insert("FIXTURE_PORT".to_owned(), options.get_port().to_string());
        credential_directory = directory.clone();
    }
    let exported = directory.join("child.env");
    let status = tokio::process::Command::new(std::env::current_exe().map_err(|_| ProcessError)?)
        .arg("export-ready")
        .envs(&environment)
        .env("FIXTURE_CREDENTIAL_DIR", credential_directory)
        .env("NEXTEST_ENV", &exported)
        .status()
        .await
        .map_err(|_| ProcessError)?;
    if !status.success() {
        return Err(ProcessError);
    }
    for line in std::fs::read_to_string(exported)
        .map_err(|_| ProcessError)?
        .lines()
    {
        let (key, value) = line.split_once('=').ok_or(ProcessError)?;
        environment.insert(key.to_owned(), value.to_owned());
    }
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| ProcessError)?;
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|_| ProcessError)?;
    let mut child = tokio::process::Command::new(binary)
        .args(arguments)
        .envs(environment)
        .spawn()
        .map_err(|_| ProcessError)?;
    let status = tokio::select! {
        status = child.wait() => status.map_err(|_| ProcessError)?,
        _ = terminate.recv() => forward_signal(&mut child, "TERM").await?,
        _ = interrupt.recv() => forward_signal(&mut child, "INT").await?,
    };
    Ok(exit_code(status))
}

async fn forward_signal(
    child: &mut tokio::process::Child,
    signal: &str,
) -> Result<ExitStatus, ProcessError> {
    if let Some(pid) = child.id() {
        let forwarded = tokio::process::Command::new("kill")
            .args(["-s", signal, &pid.to_string()])
            .status()
            .await
            .map_err(|_| ProcessError)?;
        if !forwarded.success() {
            return Err(ProcessError);
        }
    }
    child.wait().await.map_err(|_| ProcessError)
}

fn exit_code(status: ExitStatus) -> u8 {
    u8::try_from(
        status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)),
    )
    .unwrap_or(1)
}

fn prepare_credentials(directory: &Path) -> Result<(), ProcessError> {
    for role in ["admin", "migration", "runtime", "preparation"] {
        let mut entropy = [0_u8; 32];
        std::fs::File::open("/dev/urandom")
            .map_err(|_| ProcessError)?
            .read_exact(&mut entropy)
            .map_err(|_| ProcessError)?;
        let password: String = entropy
            .iter()
            .flat_map(|byte| {
                const HEX: &[u8] = b"0123456789abcdef";
                [
                    char::from(HEX[(byte >> 4) as usize]),
                    char::from(HEX[(byte & 15) as usize]),
                ]
            })
            .collect();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join(role))
            .map_err(|_| ProcessError)?;
        file.write_all(password.as_bytes())
            .map_err(|_| ProcessError)?;
    }
    Ok(())
}
