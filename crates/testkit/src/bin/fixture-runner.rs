//! Fixture tooling entry point (T010; FR-024, FR-CON-37).

use std::{
    collections::BTreeMap,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};

use sqlx::{ConnectOptions, Connection, PgConnection, postgres::PgConnectOptions};
use testkit::fixtures::{
    ownership::{Endpoint, FixtureScope},
    roles::provision_roles,
};

#[tokio::main]
async fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() >= 2 && arguments[0] == "run-child" {
        if let Ok(code) = testkit::fixtures::process::run_child(
            std::path::Path::new(&arguments[1]),
            &arguments[2..],
        )
        .await
        {
            return ExitCode::from(code);
        }
        eprintln!("fixture-runner: child launch refused");
        return ExitCode::FAILURE;
    }
    if arguments.len() == 1 && arguments[0] == "--version" {
        println!("fixture-runner {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if arguments.len() == 1 && arguments[0] == "--help" {
        println!("Usage: fixture-runner --version | --help | export-ready");
        return ExitCode::SUCCESS;
    }
    if arguments.len() == 1 && arguments[0] == "export-ready" {
        if export_ready().await.is_ok() {
            return ExitCode::SUCCESS;
        }
        eprintln!("fixture-runner: owned ready endpoint validation/export failed");
        return ExitCode::FAILURE;
    }
    if arguments.len() == 1 && arguments[0] == "lease-claim" {
        if claim_lease().is_ok() {
            return ExitCode::SUCCESS;
        }
        eprintln!("fixture-runner: live-run lease refused");
        return ExitCode::FAILURE;
    }
    if arguments.len() == 1 && arguments[0] == "shared-setup" {
        if shared_setup().await.is_ok() {
            return ExitCode::SUCCESS;
        }
        eprintln!("fixture-runner: shared setup refused");
        return ExitCode::FAILURE;
    }
    if arguments.len() == 1 && arguments[0] == "cleanup" {
        if cleanup().await.is_ok() {
            return ExitCode::SUCCESS;
        }
        eprintln!("fixture-runner: active or foreign fixture cleanup refused");
        return ExitCode::FAILURE;
    }
    eprintln!("fixture-runner: invalid invocation; use --help");
    ExitCode::from(2)
}

async fn cleanup() -> Result<(), ()> {
    let (project, run, state) = shared_scope()?;
    let project = std::env::var("FIXTURE_PROJECT").unwrap_or(project);
    let run = std::env::var("FIXTURE_RUN").unwrap_or(run);
    let state = std::env::var_os("FIXTURE_STATE_DIR").map_or(state, PathBuf::from);
    let scope = FixtureScope::new(&project, &run).map_err(|_| ())?;
    let lock = testkit::fixtures::lease::SetupLock::acquire(&state).map_err(|_| ())?;
    lock.allow_cleanup(&state, &project, &run).map_err(|_| ())?;
    let mut command = tokio::process::Command::new("docker");
    command.args(["ps", "-a", "--format", "{{.ID}}"]);
    for (key, value) in scope.postgres_labels() {
        command.args(["--filter", &format!("label={key}={value}")]);
    }
    let result = command.output().await.map_err(|_| ())?;
    if !result.status.success() {
        return Err(());
    }
    for id in String::from_utf8(result.stdout).map_err(|_| ())?.lines() {
        // Recheck each resource immediately before destructive removal.
        for (key, expected) in scope.postgres_labels() {
            if inspect(id, &format!("{{{{index .Config.Labels \"{key}\"}}}}")).await? != expected {
                return Err(());
            }
        }
        let removed = tokio::process::Command::new("docker")
            .args(["rm", "-f", "-v", id])
            .output()
            .await
            .map_err(|_| ())?;
        if !removed.status.success() {
            return Err(());
        }
    }
    Ok(())
}

fn shared_scope() -> Result<(String, String, PathBuf), ()> {
    use std::hash::{Hash, Hasher};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or(())?
        .canonicalize()
        .map_err(|_| ())?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hasher);
    let project = format!("tankhor-{:x}", hasher.finish());
    let run = if std::env::var_os("CI").is_some() {
        format!(
            "ci-{}-{}-{}",
            environment("GITHUB_RUN_ID")?,
            environment("GITHUB_RUN_ATTEMPT")?,
            environment("GITHUB_JOB")?
        )
    } else {
        "local".to_owned()
    };
    let state = root.join("target/fixtures").join(&run);
    Ok((project, run, state))
}

async fn shared_setup() -> Result<(), ()> {
    use std::io::Read;
    let (project, run, state) = shared_scope()?;
    let scope = FixtureScope::new(&project, &run).map_err(|_| ())?;
    std::fs::create_dir_all(&state).map_err(|_| ())?;
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).map_err(|_| ())?;
    let owner = nextest_owner()?;
    let _lock =
        testkit::fixtures::services::claim_run(&state, &project, &run, owner).map_err(|_| ())?;
    for role in ["admin", "migration", "runtime", "preparation"] {
        let path = state.join(role);
        if !path.exists() {
            let mut entropy = [0_u8; 32];
            std::fs::File::open("/dev/urandom")
                .map_err(|_| ())?
                .read_exact(&mut entropy)
                .map_err(|_| ())?;
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
                .open(path)
                .map_err(|_| ())?;
            file.write_all(password.as_bytes()).map_err(|_| ())?;
        }
    }
    let fixture = testkit::fixtures::postgres::PostgresFixture::start_or_reuse(
        &scope,
        "fixture_admin",
        &state.join("admin"),
    )
    .await
    .map_err(|_| ())?;
    let options = fixture.connect_options();
    let status = tokio::process::Command::new(std::env::current_exe().map_err(|_| ())?)
        .arg("export-ready")
        .env("FIXTURE_PROJECT", project)
        .env("FIXTURE_RUN", run)
        .env("FIXTURE_CONTAINER", fixture.id())
        .env("FIXTURE_HOST", options.get_host())
        .env("FIXTURE_PORT", options.get_port().to_string())
        .env("FIXTURE_CREDENTIAL_DIR", state)
        .status()
        .await
        .map_err(|_| ())?;
    if !status.success() {
        return Err(());
    }
    Ok(())
}

fn nextest_owner() -> Result<u32, ()> {
    let mut pid = std::process::id();
    loop {
        let command = std::fs::read(format!("/proc/{pid}/cmdline")).map_err(|_| ())?;
        let executable = command.split(|byte| *byte == 0).next().ok_or(())?;
        if std::path::Path::new(std::str::from_utf8(executable).map_err(|_| ())?)
            .file_name()
            .is_some_and(|name| name == "cargo-nextest")
        {
            return Ok(pid);
        }
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|_| ())?;
        let (_, fields) = stat.rsplit_once(')').ok_or(())?;
        let parent: u32 = fields
            .split_whitespace()
            .nth(1)
            .ok_or(())?
            .parse()
            .map_err(|_| ())?;
        if parent <= 1 || parent == pid {
            return Err(());
        }
        pid = parent;
    }
}

fn claim_lease() -> Result<(), ()> {
    let directory = PathBuf::from(environment("FIXTURE_STATE_DIR")?);
    let project = environment("FIXTURE_PROJECT")?;
    let run = environment("FIXTURE_RUN")?;
    let owner = environment("FIXTURE_OWNER_PID")?.parse().map_err(|_| ())?;
    testkit::fixtures::services::claim_run(&directory, &project, &run, owner).map_err(|_| ())?;
    Ok(())
}

fn environment(name: &str) -> Result<String, ()> {
    let value = std::env::var(name).map_err(|_| ())?;
    if value.is_empty() || value.contains(['\n', '\r']) {
        return Err(());
    }
    Ok(value)
}

async fn inspect(id: &str, format: &str) -> Result<String, ()> {
    let result = tokio::process::Command::new("docker")
        .args(["inspect", "--format", format, id])
        .output()
        .await
        .map_err(|_| ())?;
    if !result.status.success() {
        return Err(());
    }
    String::from_utf8(result.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|_| ())
}

fn secret(directory: &std::path::Path, role: &str) -> Result<String, ()> {
    let path = directory.join(role);
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| ())?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
        return Err(());
    }
    let password = std::fs::read_to_string(path).map_err(|_| ())?;
    if password.len() != 64 || !password.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(());
    }
    Ok(password)
}

async fn export_ready() -> Result<(), ()> {
    let scope = FixtureScope::new(
        &environment("FIXTURE_PROJECT")?,
        &environment("FIXTURE_RUN")?,
    )
    .map_err(|_| ())?;
    let id = environment("FIXTURE_CONTAINER")?;
    let host = environment("FIXTURE_HOST")?;
    let port = environment("FIXTURE_PORT")?
        .parse::<u16>()
        .map_err(|_| ())?;
    let requested = Endpoint::new(&host, port).map_err(|_| ())?;
    let inspected_host = inspect(
        &id,
        "{{(index (index .NetworkSettings.Ports \"5432/tcp\") 0).HostIp}}",
    )
    .await?;
    let inspected_port = inspect(
        &id,
        "{{(index (index .NetworkSettings.Ports \"5432/tcp\") 0).HostPort}}",
    )
    .await?
    .parse::<u16>()
    .map_err(|_| ())?;
    let inspected = Endpoint::new(&inspected_host, inspected_port).map_err(|_| ())?;
    let mut labels = BTreeMap::new();
    for key in scope.postgres_labels().keys() {
        labels.insert(
            key.clone(),
            inspect(&id, &format!("{{{{index .Config.Labels \"{key}\"}}}}")).await?,
        );
    }
    scope
        .validate(&labels, requested, inspected)
        .map_err(|_| ())?;
    if inspect(&id, "{{.State.Running}} {{.State.Paused}}").await? != "true false" {
        return Err(());
    }
    let directory = PathBuf::from(environment("FIXTURE_CREDENTIAL_DIR")?)
        .canonicalize()
        .map_err(|_| ())?;
    let password = secret(&directory, "admin")?;
    let options = PgConnectOptions::new()
        .host(&host)
        .port(port)
        .username("fixture_admin")
        .database("postgres")
        .password(&password)
        .disable_statement_logging();
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut connection = PgConnection::connect_with(&options).await.map_err(|_| ())?;
        let ready = sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&mut connection)
            .await
            .map_err(|_| ())?;
        connection.close().await.map_err(|_| ())?;
        if ready != 1 {
            return Err(());
        }
        Ok(())
    })
    .await
    .map_err(|_| ())??;
    provision_roles(&options, &directory)
        .await
        .map_err(|_| ())?;
    let passfile = write_passfile(&directory, &host, port)?;
    let url_host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host
    };
    let export = format!(
        "DATABASE_URL=postgresql://fixture_admin@{url_host}:{port}/postgres\nPGPASSFILE={}\nFIXTURE_PROJECT={}\nFIXTURE_RUN={}\nFIXTURE_CONTAINER={id}\n",
        passfile.to_str().ok_or(())?,
        environment("FIXTURE_PROJECT")?,
        environment("FIXTURE_RUN")?
    );
    let mut environment_file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(environment("NEXTEST_ENV")?)
        .map_err(|_| ())?;
    environment_file
        .write_all(export.as_bytes())
        .map_err(|_| ())
}

fn write_passfile(directory: &std::path::Path, host: &str, port: u16) -> Result<PathBuf, ()> {
    let passfile = directory.join("pgpass");
    let temporary = directory.join(format!("pgpass-{}", std::process::id()));
    let mut protected = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|_| ())?;
    for (role, username) in [
        ("admin", "fixture_admin"),
        ("migration", "tankhor_fixture_migration"),
        ("runtime", "tankhor_fixture_runtime"),
        ("preparation", "tankhor_fixture_preparation"),
    ] {
        let password = secret(directory, role)?;
        writeln!(
            protected,
            "{}:{port}:*:{username}:{password}",
            host.replace(':', "\\:")
        )
        .map_err(|_| ())?;
    }
    protected.sync_all().map_err(|_| ())?;
    std::fs::rename(&temporary, &passfile).map_err(|_| ())?;
    Ok(passfile)
}
