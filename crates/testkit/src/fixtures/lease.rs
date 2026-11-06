//! Live nextest-owner lease, serialized claims and verified stale reclaim (T020).

use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

/// Typed, nonsecret refusal: malformed state never implies permission to reuse services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseError {
    State,
    Owner,
    Busy,
    Foreign,
}

impl std::fmt::Display for LeaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "fixture lease refusal: {self:?}")
    }
}
impl std::error::Error for LeaseError {}

/// Kernel lock is held across setup and released even if the process exits unexpectedly.
pub struct SetupLock {
    _file: File,
}

impl SetupLock {
    /// Check exact scope and prove that the recorded owner is no longer live.
    ///
    /// # Errors
    /// Refuses active runs and missing, foreign, corrupt or unprotected lease metadata.
    pub fn allow_cleanup(
        &self,
        directory: &Path,
        project: &str,
        run: &str,
    ) -> Result<(), LeaseError> {
        let path = directory.join("live-run.lease");
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| LeaseError::State)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(LeaseError::State);
        }
        let contents = std::fs::read_to_string(path).map_err(|_| LeaseError::State)?;
        let fields: Vec<_> = contents.lines().collect();
        if fields.len() != 5 || fields[0] != project || fields[1] != run {
            return Err(LeaseError::Foreign);
        }
        let pid = fields[2].parse().map_err(|_| LeaseError::State)?;
        let start = fields[3].parse().map_err(|_| LeaseError::State)?;
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|_| LeaseError::Owner)?;
        if fields[4] == boot.trim() && process_start(pid)? == Some(start) {
            return Err(LeaseError::Busy);
        }
        Ok(())
    }

    /// Acquire the private repository state lock before reading or writing lease metadata.
    ///
    /// # Errors
    /// Refuses unsafe state permissions, symlink paths or an already-running setup.
    pub fn acquire(directory: &Path) -> Result<Self, LeaseError> {
        let metadata = std::fs::symlink_metadata(directory).map_err(|_| LeaseError::State)?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o777 != 0o700 {
            return Err(LeaseError::State);
        }
        let path = directory.join("setup.lock");
        if path.is_symlink() {
            return Err(LeaseError::State);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .map_err(|_| LeaseError::State)?;
        if file
            .metadata()
            .map_err(|_| LeaseError::State)?
            .permissions()
            .mode()
            & 0o777
            != 0o600
        {
            return Err(LeaseError::State);
        }
        file.try_lock().map_err(|_| LeaseError::Busy)?;
        Ok(Self { _file: file })
    }

    /// Persist a claim tied to the owner's PID, start tick and current kernel boot ID.
    ///
    /// # Errors
    /// Refuses active independent runs, foreign/corrupt metadata and unverifiable owners.
    pub fn claim(
        &self,
        directory: &Path,
        project: &str,
        run: &str,
        pid: u32,
    ) -> Result<(), LeaseError> {
        super::ownership::FixtureScope::new(project, run).map_err(|_| LeaseError::Foreign)?;
        let start = process_start(pid)?.ok_or(LeaseError::Owner)?;
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map_err(|_| LeaseError::Owner)?;
        let boot = boot.trim();
        let path = directory.join("live-run.lease");
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.permissions().mode() & 0o777 != 0o600 {
                    return Err(LeaseError::State);
                }
                let contents = std::fs::read_to_string(&path).map_err(|_| LeaseError::State)?;
                let fields: Vec<_> = contents.lines().collect();
                if fields.len() != 5 || fields[0] != project || fields[1] != run {
                    return Err(LeaseError::Foreign);
                }
                let previous_pid: u32 = fields[2].parse().map_err(|_| LeaseError::State)?;
                let previous_start: u64 = fields[3].parse().map_err(|_| LeaseError::State)?;
                if fields[4] == boot && process_start(previous_pid)? == Some(previous_start) {
                    if previous_pid == pid && previous_start == start {
                        return Ok(());
                    }
                    return Err(LeaseError::Busy);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(LeaseError::State),
        }
        let temporary = directory.join(format!("lease-{}", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| LeaseError::State)?;
        writeln!(file, "{project}\n{run}\n{pid}\n{start}\n{boot}")
            .map_err(|_| LeaseError::State)?;
        file.sync_all().map_err(|_| LeaseError::State)?;
        std::fs::rename(temporary, path).map_err(|_| LeaseError::State)
    }
}

fn process_start(pid: u32) -> Result<Option<u64>, LeaseError> {
    let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(LeaseError::Owner),
    };
    // comm may contain spaces and ')'; fields after its last ')' begin with state (3).
    let (_, fields) = stat.rsplit_once(')').ok_or(LeaseError::Owner)?;
    let fields: Vec<_> = fields.split_whitespace().collect();
    if matches!(fields.first(), Some(&"Z" | &"X")) {
        return Ok(None);
    }
    fields
        .get(19)
        .ok_or(LeaseError::Owner)?
        .parse()
        .map(Some)
        .map_err(|_| LeaseError::Owner)
}
