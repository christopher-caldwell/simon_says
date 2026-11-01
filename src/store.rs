//! Durable records and exclusive single-operator access for a workflow run.
use crate::{
    routing::{Gate, State, Status},
    Snapshot,
};
use anyhow::{bail, ensure, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub version: u32,
    pub run_id: String,
    pub snapshot_digest: String,
    pub state: State,
    pub next_action: u64,
    /// Opaque serialized data owned and validated by the Git checkpoint layer.
    pub worktree: serde_json::Value,
}

pub struct RunStore {
    root: PathBuf,
    _lock: File,
}
impl RunStore {
    pub fn create(root: &Path, snapshot: &Snapshot, record: &RunRecord) -> Result<Self> {
        ensure!(
            !root.exists(),
            "run directory already exists: {}",
            root.display()
        );
        fs::create_dir(root).context("create run directory")?;
        let root = root.canonicalize()?;
        if let Some(parent) = root.parent() {
            File::open(parent)?.sync_all()?;
        }
        fs::create_dir(root.join("actions"))?;
        let store = Self::lock(&root)?;
        ensure!(
            record.version == 1 && record.snapshot_digest == snapshot.sha256,
            "invalid initial record"
        );
        store.write_json_new("snapshot.json", snapshot)?;
        store.save(record)?;
        Ok(store)
    }
    pub fn open(root: &Path) -> Result<Self> {
        Self::lock(&root.canonicalize().context("run directory not found")?)
    }
    fn lock(root: &Path) -> Result<Self> {
        let path = root.join("lock");
        if let Ok(meta) = fs::symlink_metadata(&path) {
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "unsafe run lock"
            );
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let lock = options.open(path)?;
        lock.try_lock_exclusive()
            .context("run is already open by another process")?;
        let store = Self {
            root: root.to_owned(),
            _lock: lock,
        };
        store.verify_lock()?;
        Ok(store)
    }
    pub fn verify_lock(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let held = self._lock.metadata()?;
            let named = fs::symlink_metadata(self.root.join("lock"))?;
            ensure!(
                named.is_file()
                    && !named.file_type().is_symlink()
                    && held.dev() == named.dev()
                    && held.ino() == named.ino()
                    && held.nlink() == 1,
                "run lock identity changed"
            );
        }
        Ok(())
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn load(&self) -> Result<(RunRecord, Snapshot)> {
        self.verify_lock()?;
        for name in ["state.json", "snapshot.json"] {
            let meta = fs::symlink_metadata(self.root.join(name))?;
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "unsafe run record {name}"
            );
        }
        let record: RunRecord = serde_json::from_slice(&fs::read(self.root.join("state.json"))?)?;
        ensure!(record.version == 1, "unsupported run state version");
        let snapshot: Snapshot =
            serde_json::from_slice(&fs::read(self.root.join("snapshot.json"))?)?;
        snapshot.verify()?;
        ensure!(
            snapshot.sha256 == record.snapshot_digest,
            "run authority digest mismatch"
        );
        Ok((record, snapshot))
    }
    /// Restore the in-memory authority and running state after detecting tampering.
    /// This is accidental-tamper detection, not a same-user security boundary.
    pub fn restore_trusted(&self, record: &RunRecord, snapshot: &Snapshot) -> Result<()> {
        self.verify_lock()?;
        ensure!(
            record.state.status == Status::Running && record.snapshot_digest == snapshot.sha256,
            "invalid authority restoration"
        );
        atomic_json(&self.root.join("snapshot.json"), snapshot)?;
        atomic_json(&self.root.join("state.json"), record)?;
        Ok(())
    }
    pub fn save(&self, record: &RunRecord) -> Result<()> {
        self.verify_lock()?;
        atomic_json(&self.root.join("state.json"), record)
    }
    fn write_json_new(&self, name: &str, value: &impl Serialize) -> Result<()> {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.root.join(name))?;
        serde_json::to_writer_pretty(&mut f, value)?;
        f.write_all(b"\n")?;
        f.sync_all()?;
        sync_dir(&self.root)?;
        Ok(())
    }
    /// Persist action identity before Git checkout, filters or other preparation.
    pub fn prepare_action(&self, record: &mut RunRecord, id: &str) -> Result<()> {
        ensure!(record.state.status == Status::Ready, "run is not ready");
        ensure!(
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid action id"
        );
        let next_action = record
            .next_action
            .checked_add(1)
            .context("action counter exhausted")?;
        record.state.status = Status::Preparing;
        record.state.action_id = Some(id.to_owned());
        record.next_action = next_action;
        self.save(record)
    }
    /// Reserve an action directory and a writable scratch area before preparing its sandbox.
    pub fn reserve(&self, id: &str) -> Result<PathBuf> {
        ensure!(
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid action id"
        );
        let dir = self.root.join("actions").join(id);
        fs::create_dir(&dir).context("reserve action directory")?;
        let scratch = dir.join("scratch");
        fs::create_dir(&scratch)?;
        sync_dir(&dir)?;
        sync_dir(&self.root.join("actions"))?;
        Ok(scratch)
    }
    /// Records packet and invocation plan before atomically setting Running. No subprocess has been invoked.
    pub fn begin(
        &self,
        record: &mut RunRecord,
        packet: &serde_json::Value,
        plan: &impl Serialize,
    ) -> Result<PathBuf> {
        ensure!(
            record.state.status == Status::Preparing,
            "run has not prepared an action"
        );
        let id = packet
            .get("action_id")
            .and_then(|v| v.as_str())
            .context("packet lacks action identity")?;
        ensure!(
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid action id"
        );
        ensure!(
            record.state.action_id.as_deref() == Some(id),
            "packet identity does not match prepared action"
        );
        let dir = self.root.join("actions").join(id);
        if dir.exists() {
            let metadata = fs::symlink_metadata(&dir)?;
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "unsafe action directory"
            );
            let entries = fs::read_dir(&dir)?.collect::<std::result::Result<Vec<_>, _>>()?;
            ensure!(
                entries.len() == 1
                    && entries[0].file_name() == "scratch"
                    && entries[0].file_type()?.is_dir(),
                "reserved action directory is not empty"
            );
        } else {
            fs::create_dir(&dir).context("create action directory")?;
        }
        write_new(&dir.join("packet.json"), packet)?;
        write_new(&dir.join("invocation.json"), plan)?;
        sync_dir(&dir)?;
        sync_dir(&self.root.join("actions"))?;
        record.state.start(id.to_owned())?;
        self.save(record)?;
        Ok(dir)
    }
    pub fn check_resumable(record: &RunRecord) -> Result<()> {
        match record.state.status {
            Status::Initializing => bail!("run initialization was interrupted; it cannot be resumed"),
            Status::Preparing => bail!("action {:?} preparation is uncertain after interruption or failure; explicit reset is required; external side effects are not undone", record.state.action_id),
            Status::Running => bail!("action {:?} is uncertain after interruption or failure; explicit reset is required; external side effects are not undone", record.state.action_id),
            Status::Stopped => bail!("run stopped: {}; explicit recovery is required", record.state.stop_reason.as_deref().unwrap_or("unspecified")),
            Status::AwaitingApproval => bail!("run awaits explicit approval of its recorded accepted checkpoint and workflow digest"),
            Status::ApprovalChecking => bail!("approval verification is uncertain; explicit reset is required before reapproval"),
            Status::Complete => bail!("run is already complete"),
            Status::Ready => Ok(()),
        }
    }
}
fn write_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut f, value)?;
    f.write_all(b"\n")?;
    f.sync_all()?;
    Ok(())
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("state has no parent")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    sync_dir(parent)?;
    Ok(())
}
fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

/// Which previously confirmed commit an explicit recovery would restore.
pub fn recovery_commit(state: &State) -> &str {
    match state.gate {
        Gate::Worker => state
            .candidate_commit
            .as_deref()
            .unwrap_or(&state.accepted_commit),
        Gate::Reviewer => state
            .candidate_commit
            .as_deref()
            .unwrap_or(&state.accepted_commit),
        Gate::Unblocker => state
            .candidate_commit
            .as_deref()
            .unwrap_or(&state.accepted_commit),
    }
}
