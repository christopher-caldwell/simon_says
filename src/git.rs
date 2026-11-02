//! Controlled linked worktrees and exact Git checkpoint checks.
//!
//! These operations protect the user's source checkout; they do not sandbox
//! subprocesses and cannot undo external side effects.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Worktree {
    pub source: PathBuf,
    pub run_dir: PathBuf,
    pub path: PathBuf,
    pub git_dir: PathBuf,
    pub baseline: String,
    /// Persisted linked-worktree admin directory identity. Legacy records lack this.
    #[serde(default)]
    pub git_identity: Option<WorktreeIdentity>,
    #[serde(default)]
    pub checkout_identity: Option<WorktreeIdentity>,
    #[serde(default)]
    pub source_identity: Option<WorktreeIdentity>,
    #[serde(default)]
    pub common_identity: Option<WorktreeIdentity>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorktreeIdentity {
    pub device: u64,
    pub inode: u64,
}
impl WorktreeIdentity {
    pub fn capture(path: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "worktree admin directory is unsafe"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("worktree ownership identity is unsupported on this platform")
        }
    }
}

fn git_command(repo: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-c")
        .arg("core.hooksPath=/dev/null")
        .arg("-C")
        .arg(repo);
    cmd.env("GIT_NO_REPLACE_OBJECTS", "1");
    // A parent process must not redirect this command to a different repository.
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
        "GIT_NAMESPACE",
    ] {
        cmd.env_remove(key);
    }
    cmd
}
fn git(repo: &Path, args: &[&str]) -> Result<Output> {
    let output =
        crate::git_process::run(git_command(repo).args(args), None).context("execute git")?;
    ensure!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output)
}
fn git_text(repo: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8(git(repo, args)?.stdout)
        .context("git returned non-UTF-8 output")?
        .trim()
        .to_owned())
}
fn canonical(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("canonicalize {}", path.display()))
}
fn path_arg(path: &Path) -> Result<&str> {
    path.to_str().context("Git worktree path must be UTF-8")
}
fn exact_commit(repo: &Path, id: &str) -> Result<String> {
    ensure!(
        (id.len() == 40 || id.len() == 64)
            && id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "checkpoint must be a full lowercase Git object ID"
    );
    let actual = git_text(
        repo,
        &["rev-parse", "--verify", &format!("{id}^{{commit}}")],
    )?;
    ensure!(actual == id, "checkpoint is not an exact commit ID");
    Ok(actual)
}
fn ancestor(repo: &Path, older: &str, newer: &str) -> Result<()> {
    exact_commit(repo, older)?;
    exact_commit(repo, newer)?;
    git(repo, &["merge-base", "--is-ancestor", older, newer])
        .context("checkpoint is not an ancestor")?;
    Ok(())
}
fn detached(repo: &Path) -> Result<()> {
    ensure!(
        git_text(repo, &["rev-parse", "--abbrev-ref", "HEAD"])? == "HEAD",
        "worktree HEAD must remain detached"
    );
    Ok(())
}
fn no_hidden_index_changes(repo: &Path) -> Result<()> {
    let entries = git(repo, &["ls-files", "-v", "-z"])?;
    for entry in entries.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        ensure!(
            !entry[0].is_ascii_lowercase() && entry[0] != b'S',
            "worktree has assume-unchanged or skip-worktree index entries"
        );
    }
    Ok(())
}
// Git's stat cache may report a tracked file as clean without reading its
// bytes. Hash worktree contents independently, applying Git's clean filters.
fn verify_tracked_contents(repo: &Path) -> Result<()> {
    let index = git(repo, &["ls-files", "--stage", "-z"])?;
    let mut batch = Vec::new();
    for entry in index
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let separator = entry
            .iter()
            .position(|byte| *byte == b'\t')
            .context("invalid Git index entry")?;
        let header =
            std::str::from_utf8(&entry[..separator]).context("invalid Git index metadata")?;
        let path = std::str::from_utf8(&entry[separator + 1..])
            .context("non-UTF-8 tracked paths are not supported")?;
        let mut fields = header.split_whitespace();
        let mode = fields.next().context("missing tracked file mode")?;
        let oid = fields.next().context("missing tracked object")?;
        ensure!(
            fields.next() == Some("0") && fields.next().is_none(),
            "unmerged Git index entry: {path}"
        );
        let full_path = repo.join(path);
        let mut current = repo.to_path_buf();
        for component in Path::new(path).components() {
            current.push(component);
            let metadata = fs::symlink_metadata(&current)
                .with_context(|| format!("missing tracked path {path}"))?;
            if current != full_path {
                ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "unsafe tracked path parent: {path}"
                );
            }
        }
        let metadata = fs::symlink_metadata(&current)
            .with_context(|| format!("missing tracked path {path}"))?;
        match mode {
            "100644" | "100755" => {
                ensure!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "tracked file is not regular: {path}"
                );
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let executable = metadata.permissions().mode() & 0o111 != 0;
                    ensure!(
                        executable == (mode == "100755"),
                        "tracked executable mode changed: {path}"
                    );
                }
                // --stdin-paths is line-based and recognizes Git quoted paths.
                // Fall back to a direct argument for paths requiring quoting.
                if path.contains(['\n', '\r', '\"', '\\']) {
                    let actual = git_text(
                        repo,
                        &["hash-object", &format!("--path={path}"), "--", path],
                    )?;
                    ensure!(actual == oid, "tracked file content changed: {path}");
                } else {
                    batch.push((path, oid));
                }
            }
            "120000" => {
                ensure!(
                    metadata.file_type().is_symlink(),
                    "tracked symlink changed type: {path}"
                );
                let target = fs::read_link(&current)
                    .with_context(|| format!("read tracked symlink {path}"))?;
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStrExt;
                    let expected = git(repo, &["cat-file", "blob", oid])?.stdout;
                    ensure!(
                        target.as_os_str().as_bytes() == expected,
                        "tracked symlink target changed: {path}"
                    );
                }
                #[cfg(not(unix))]
                {
                    let expected = git(repo, &["cat-file", "blob", oid])?.stdout;
                    ensure!(
                        target.to_string_lossy().as_bytes() == expected,
                        "tracked symlink target changed: {path}"
                    );
                }
            }
            // Nested repositories have their own stat caches and require a separate
            // content audit; fail closed until that is implemented.
            "160000" => bail!("submodules are not supported by the checkpoint validator: {path}"),
            _ => bail!("unsupported tracked file mode {mode} for {path}"),
        }
    }
    // Bound batches. File-backed input/output in the shared Git runner avoids
    // deadlock and places a deadline on configured clean filters.
    for chunk in batch.chunks(128) {
        let input = chunk
            .iter()
            .map(|(path, _)| format!("{path}\n"))
            .collect::<String>();
        let output = crate::git_process::run(
            git_command(repo).args(["hash-object", "--stdin-paths"]),
            Some(input.as_bytes()),
        )
        .context("git hash-object")?;
        ensure!(
            output.status.success(),
            "git hash-object failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let hashes = std::str::from_utf8(&output.stdout).context("invalid git hash output")?;
        let mut actual = hashes.lines();
        for (path, oid) in chunk {
            ensure!(
                actual.next() == Some(*oid),
                "tracked file content changed: {path}"
            );
        }
        ensure!(actual.next().is_none(), "unexpected git hash output");
    }
    Ok(())
}
fn clean(repo: &Path) -> Result<()> {
    no_hidden_index_changes(repo)?;
    let output = git(
        repo,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
    )?;
    ensure!(
        output.stdout.is_empty(),
        "worktree is dirty: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    verify_tracked_contents(repo)?;
    Ok(())
}
// An ignored file in a managed checkout is not part of the Git candidate,
// but a later Worker could still read or execute it. Reviewers get a fresh
// checkout and therefore could not inspect those bytes. Reject all ignored
// content in managed worktrees. Empty directory trees carry no candidate bytes
// and are permitted; never apply this rule to the user's source
// checkout, where ignored files are expected and remain untouched.
fn only_empty_ignored_directories(repo: &Path, status: &[u8]) -> Result<bool> {
    use std::os::unix::ffi::OsStrExt;
    let mut examined = 0usize;
    for entry in status.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let Some(name) = entry.strip_prefix(b"!! ") else {
            return Ok(false);
        };
        let relative = Path::new(std::ffi::OsStr::from_bytes(name));
        let mut path = repo.to_path_buf();
        for component in relative.components() {
            let std::path::Component::Normal(part) = component else {
                return Ok(false);
            };
            path.push(part);
            let meta = std::fs::symlink_metadata(&path)?;
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Ok(false);
            }
        }
        let mut pending = vec![path];
        while let Some(dir) = pending.pop() {
            examined += 1;
            ensure!(examined <= 4096, "too many ignored directories to verify");
            for item in std::fs::read_dir(dir)? {
                let item = item?;
                let meta = std::fs::symlink_metadata(item.path())?;
                if !meta.is_dir() || meta.file_type().is_symlink() {
                    return Ok(false);
                }
                pending.push(item.path());
            }
        }
    }
    Ok(true)
}
fn clean_managed(repo: &Path) -> Result<()> {
    clean(repo)?;
    let output = git(
        repo,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored=matching",
            "--ignore-submodules=none",
        ],
    )?;
    ensure!(
        only_empty_ignored_directories(repo, &output.stdout)?,
        "managed worktree contains ignored or untracked files; preserve them and reconcile explicitly"
    );
    Ok(())
}

/// Frozen repository identity and pinned baseline recorded before checkout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitializationIntent {
    pub kind: String,
    pub phase: InitializationPhase,
    pub source: PathBuf,
    pub common_dir: PathBuf,
    pub run_root: PathBuf,
    pub baseline: String,
    #[cfg(unix)]
    pub common_dev: u64,
    #[cfg(unix)]
    pub common_ino: u64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InitializationPhase {
    Reserved,
    Creating,
}
impl InitializationIntent {
    pub fn capture(repo: &Path, run_dir: &Path) -> Result<Self> {
        let source = canonical(Path::new(&git_text(
            repo,
            &["rev-parse", "--show-toplevel"],
        )?))?;
        clean(&source).context("source checkout must be clean")?;
        let baseline = git_text(&source, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        exact_commit(&source, &baseline)?;
        let common_dir = canonical(Path::new(&git_text(
            &source,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?))?;
        #[cfg(unix)]
        let metadata = {
            use std::os::unix::fs::MetadataExt;
            let meta = fs::metadata(&common_dir)?;
            (meta.dev(), meta.ino())
        };
        let run_root = if run_dir.exists() {
            canonical(run_dir)?
        } else {
            let name = run_dir
                .file_name()
                .context("run directory needs a final path component")?;
            let parent = run_dir
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            canonical(parent)?.join(name)
        };
        ensure!(
            !run_root.starts_with(&source),
            "run directory must be outside source checkout"
        );
        ensure!(
            !run_root.starts_with(&common_dir),
            "run directory must be outside Git's common directory"
        );
        Ok(Self {
            kind: "initialization".into(),
            phase: InitializationPhase::Reserved,
            source,
            common_dir,
            run_root,
            baseline,
            #[cfg(unix)]
            common_dev: metadata.0,
            #[cfg(unix)]
            common_ino: metadata.1,
        })
    }
    fn verify_source(&self, require_clean: bool) -> Result<()> {
        ensure!(
            self.kind == "initialization",
            "invalid initialization intent"
        );
        ensure!(
            canonical(&self.source)? == self.source
                && canonical(&self.common_dir)? == self.common_dir,
            "initialization source identity changed"
        );
        let top = canonical(Path::new(&git_text(
            &self.source,
            &["rev-parse", "--show-toplevel"],
        )?))?;
        let common = canonical(Path::new(&git_text(
            &self.source,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?))?;
        ensure!(
            top == self.source && common == self.common_dir,
            "initialization repository identity changed"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&common)?;
            ensure!(
                metadata.dev() == self.common_dev && metadata.ino() == self.common_ino,
                "initialization Git directory identity changed"
            );
        }
        exact_commit(&self.source, &self.baseline)?;
        if require_clean {
            ensure!(
                git_text(&self.source, &["rev-parse", "--verify", "HEAD^{commit}"])?
                    == self.baseline,
                "source HEAD changed since initialization"
            );
            clean(&self.source).context("source checkout must still be clean")?;
        }
        Ok(())
    }
    fn verify_target(&self, run_dir: &Path) -> Result<PathBuf> {
        let root = canonical(run_dir)?;
        ensure!(
            root == self.run_root,
            "initialization run directory changed"
        );
        ensure!(
            !root.starts_with(&self.source),
            "run directory must be outside source checkout"
        );
        ensure!(
            !root.starts_with(&self.common_dir),
            "run directory must be outside Git's common directory"
        );
        Ok(root.join("worktree"))
    }
    pub fn target_exists(&self, run_dir: &Path) -> Result<bool> {
        let path = self.verify_target(run_dir)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    pub fn verify_absent_and_unregistered(&self, run_dir: &Path) -> Result<()> {
        self.verify_source(true)?;
        let target = self.verify_target(run_dir)?;
        ensure!(!self.target_exists(run_dir)?, "worktree target exists");
        let listing = git(&self.source, &["worktree", "list", "--porcelain", "-z"])?;
        let field = format!("worktree {}", path_arg(&target)?);
        ensure!(
            !listing
                .stdout
                .split(|b| *b == 0)
                .any(|v| v == field.as_bytes()),
            "missing worktree target is still registered; manual reconciliation required"
        );
        Ok(())
    }
    pub fn create(&self, run_dir: &Path) -> Result<Worktree> {
        self.verify_absent_and_unregistered(run_dir)?;
        Worktree::create_at(
            self.source.clone(),
            canonical(run_dir)?,
            "worktree",
            &self.baseline,
        )
    }
    pub fn adopt(&self, run_dir: &Path) -> Result<Worktree> {
        self.verify_source(false)?;
        let target = self.verify_target(run_dir)?;
        let metadata =
            fs::symlink_metadata(&target).context("initialization worktree target missing")?;
        ensure!(
            metadata.is_dir()
                && !metadata.file_type().is_symlink()
                && canonical(&target)? == target,
            "unsafe initialization worktree target"
        );
        let git_dir = canonical(Path::new(&git_text(
            &target,
            &["rev-parse", "--absolute-git-dir"],
        )?))?;
        let worktree = Worktree::record(
            self.source.clone(),
            canonical(run_dir)?,
            target,
            git_dir,
            self.baseline.clone(),
        )?;
        worktree.verify_candidate(&self.baseline, &self.baseline)?;
        worktree.verify_pristine()?;
        Ok(worktree)
    }
}
impl Worktree {
    fn record(
        source: PathBuf,
        run_dir: PathBuf,
        path: PathBuf,
        git_dir: PathBuf,
        baseline: String,
    ) -> Result<Self> {
        let common = canonical(Path::new(&git_text(
            &source,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?))?;
        Ok(Self {
            git_identity: Some(WorktreeIdentity::capture(&git_dir)?),
            checkout_identity: Some(WorktreeIdentity::capture(&path)?),
            source_identity: Some(WorktreeIdentity::capture(&source)?),
            common_identity: Some(WorktreeIdentity::capture(&common)?),
            source,
            run_dir,
            path,
            git_dir,
            baseline,
        })
    }
    /// Create `run_dir/worktree` from a clean checkout, without changing it.
    /// The run directory must already exist, be outside the source tree, and
    /// be reserved for this run by the caller.
    pub fn create(repo: &Path, run_dir: &Path) -> Result<Self> {
        InitializationIntent::capture(repo, run_dir)?.create(run_dir)
    }
    fn create_at(source: PathBuf, run_dir: PathBuf, name: &str, baseline: &str) -> Result<Self> {
        exact_commit(&source, baseline)?;
        let path = run_dir.join(name);
        ensure!(
            fs::symlink_metadata(&path).is_err(),
            "worktree target already exists: {}",
            path.display()
        );
        git(
            &source,
            &[
                "worktree",
                "add",
                "--detach",
                "--",
                path_arg(&path)?,
                baseline,
            ],
        )?;
        let path = canonical(&path)?;
        let git_dir = canonical(Path::new(&git_text(
            &path,
            &["rev-parse", "--absolute-git-dir"],
        )?))?;
        let worktree = Self::record(source, run_dir, path, git_dir, baseline.to_owned())?;
        worktree.verify_owned()?;
        worktree.verify_candidate(baseline, baseline)?;
        Ok(worktree)
    }
    /// Create a separate detached review checkout at an already verified candidate.
    /// `review_dir` is an existing, caller-owned empty directory for one action.
    pub fn create_review(&self, candidate: &str, review_dir: &Path) -> Result<Self> {
        self.verify_candidate(candidate, &self.baseline)?;
        let review_dir = canonical(review_dir)?;
        ensure!(
            review_dir.starts_with(&self.run_dir)
                && review_dir != self.run_dir
                && !review_dir.starts_with(&self.path),
            "review directory must be inside run directory and outside the worker checkout"
        );
        Self::create_at(self.source.clone(), review_dir, "worktree", candidate)
    }
    /// Fail closed if this record no longer points to the originally registered worktree.
    pub fn verify_owned(&self) -> Result<()> {
        let source = canonical(&self.source)?;
        let run_dir = canonical(&self.run_dir)?;
        let path = canonical(&self.path)?;
        ensure!(
            source == self.source && run_dir == self.run_dir && path == self.path,
            "worktree paths changed"
        );
        ensure!(
            path == run_dir.join("worktree") && !path.starts_with(&source),
            "worktree is outside its controlled directory"
        );
        let check_identity = |path: &Path,
                              recorded: &Option<WorktreeIdentity>,
                              what: &str|
         -> Result<()> {
            let recorded = recorded.as_ref().with_context(|| format!("legacy worktree lacks recorded {what} identity; manual reconciliation required"))?;
            ensure!(
                &WorktreeIdentity::capture(path)? == recorded,
                "{what} identity changed"
            );
            Ok(())
        };
        check_identity(&source, &self.source_identity, "source directory")?;
        check_identity(&path, &self.checkout_identity, "checkout directory")?;
        let meta = fs::symlink_metadata(path.join(".git"))?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "linked worktree .git file missing or unsafe"
        );
        let actual_git_dir = canonical(Path::new(&git_text(
            &path,
            &["rev-parse", "--absolute-git-dir"],
        )?))?;
        ensure!(
            actual_git_dir == self.git_dir,
            "worktree Git identity changed"
        );
        let recorded_identity = self.git_identity.as_ref().context("legacy worktree lacks recorded admin-directory identity; manual reconciliation required")?;
        ensure!(
            &WorktreeIdentity::capture(&actual_git_dir)? == recorded_identity,
            "worktree admin-directory identity changed"
        );
        let source_common = canonical(Path::new(&git_text(
            &source,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?))?;
        check_identity(
            &source_common,
            &self.common_identity,
            "Git common directory",
        )?;
        let work_common = canonical(Path::new(&git_text(
            &path,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?))?;
        ensure!(
            source_common == work_common
                && self.git_dir.starts_with(source_common.join("worktrees")),
            "worktree belongs to a different repository"
        );
        ensure!(
            !run_dir.starts_with(&source_common),
            "run directory must be outside Git's common directory"
        );
        let registered = git(&source, &["worktree", "list", "--porcelain", "-z"])?;
        let record = format!("worktree {}", path_arg(&path)?);
        ensure!(
            registered
                .stdout
                .split(|b| *b == 0)
                .any(|field| field == record.as_bytes()),
            "worktree is not registered in source repository"
        );
        exact_commit(&source, &self.baseline)?;
        Ok(())
    }
    pub fn head(&self) -> Result<String> {
        self.verify_owned()?;
        git_text(&self.path, &["rev-parse", "--verify", "HEAD^{commit}"])
    }
    pub fn verify_clean(&self) -> Result<()> {
        self.verify_owned()?;
        clean_managed(&self.path)
    }
    /// Reject ignored as well as untracked or tracked mutations in a fresh review checkout.
    pub fn verify_pristine(&self) -> Result<()> {
        self.verify_owned()?;
        let status = git(
            &self.path,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored=matching",
                "--ignore-submodules=none",
            ],
        )?;
        ensure!(
            only_empty_ignored_directories(&self.path, &status.stdout)?,
            "review checkout has tracked, untracked or ignored changes"
        );
        Ok(())
    }
    /// Verify HEAD, ancestry and clean status before a candidate can be reviewed.
    pub fn verify_candidate(&self, candidate: &str, ancestor_commit: &str) -> Result<()> {
        self.verify_owned()?;
        detached(&self.path)?;
        exact_commit(&self.path, candidate)?;
        ancestor(&self.path, &self.baseline, ancestor_commit)?;
        ancestor(&self.path, ancestor_commit, candidate)?;
        ensure!(
            git_text(&self.path, &["rev-parse", "--verify", "HEAD^{commit}"])? == candidate,
            "candidate does not match worktree HEAD"
        );
        clean_managed(&self.path)
    }
    /// Checkpoint a direct role's tracked changes and declared new outputs.
    /// Never add unrelated untracked or ignored files.
    pub fn checkpoint_outputs(&self, checkpoint: &str, outputs: &[String]) -> Result<String> {
        self.verify_owned()?;
        detached(&self.path)?;
        no_hidden_index_changes(&self.path)?;
        let head = git_text(&self.path, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        ancestor(&self.path, &self.baseline, checkpoint)?;
        ancestor(&self.path, checkpoint, &head)?;
        let ignored = git(
            &self.path,
            &[
                "ls-files",
                "--others",
                "--ignored",
                "--exclude-standard",
                "-z",
            ],
        )?;
        ensure!(
            ignored.stdout.is_empty(),
            "managed worktree contains ignored or untracked files; preserve them and reconcile explicitly"
        );
        let untracked = git(
            &self.path,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )?;
        let added = git(
            &self.path,
            &[
                "diff",
                "--cached",
                "--no-renames",
                "--name-only",
                "--diff-filter=A",
                "-z",
            ],
        )?;
        for path in untracked
            .stdout
            .split(|b| *b == 0)
            .chain(added.stdout.split(|b| *b == 0))
            .filter(|p| !p.is_empty())
        {
            ensure!(
                outputs.iter().any(|output| output.as_bytes() == path),
                "untracked or newly staged file is not a declared output: {:?}",
                String::from_utf8_lossy(path)
            );
        }
        git(&self.path, &["add", "-u", "--", "."])?;
        if !outputs.is_empty() {
            let mut args = vec!["--literal-pathspecs", "add", "--"];
            args.extend(outputs.iter().map(String::as_str));
            git(&self.path, &args)?;
        }
        let changes = git(&self.path, &["diff", "--cached", "--name-only", "-z"])?;
        if !changes.stdout.is_empty() {
            git(
                &self.path,
                &[
                    "-c",
                    "user.name=Simon Says",
                    "-c",
                    "user.email=simon-says@localhost",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-m",
                    "Record workflow action",
                ],
            )?;
        }
        let candidate = git_text(&self.path, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        self.verify_candidate(&candidate, checkpoint)?;
        Ok(candidate)
    }
    /// Explicitly discard tracked changes in this verified worktree and restore
    /// a known checkpoint. Any untracked *or ignored* files cause refusal: Git
    /// reset may otherwise overwrite them. Never removes untracked files.
    pub fn reset_to(&self, checkpoint: &str) -> Result<()> {
        self.verify_owned()?;
        detached(&self.path)?;
        no_hidden_index_changes(&self.path)?;
        ancestor(&self.path, &self.baseline, checkpoint)?;
        let status = git(
            &self.path,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored=matching",
                "--ignore-submodules=none",
            ],
        )?;
        for entry in status.stdout.split(|b| *b == 0) {
            if entry.starts_with(b"??") || entry.starts_with(b"!!") {
                bail!("reset refused: untracked or ignored files require manual reconciliation");
            }
        }
        git(&self.path, &["reset", "--hard", checkpoint])?;
        self.verify_candidate(checkpoint, &self.baseline)
    }
}
