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
}

fn git(repo: &Path, args: &[&str]) -> Result<Output> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(args);
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
    let output = cmd.output().context("execute git")?;
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
fn clean(repo: &Path) -> Result<()> {
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
    Ok(())
}
impl Worktree {
    /// Create `run_dir/worktree` from a clean checkout, without changing it.
    /// The run directory must already exist, be outside the source tree, and
    /// be reserved for this run by the caller.
    pub fn create(repo: &Path, run_dir: &Path) -> Result<Self> {
        let source = canonical(Path::new(&git_text(
            repo,
            &["rev-parse", "--show-toplevel"],
        )?))?;
        clean(&source).context("source checkout must be clean")?;
        let baseline = git_text(&source, &["rev-parse", "--verify", "HEAD^{commit}"])?;
        let run_dir = canonical(run_dir)?;
        ensure!(
            !run_dir.starts_with(&source),
            "run directory must be outside the source checkout"
        );
        Self::create_at(source, run_dir, "worktree", &baseline)
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
        let worktree = Self {
            source,
            run_dir,
            path,
            git_dir,
            baseline: baseline.to_owned(),
        };
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
        let source_common = canonical(Path::new(&git_text(
            &source,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?))?;
        let work_common = canonical(Path::new(&git_text(
            &path,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?))?;
        ensure!(
            source_common == work_common
                && self.git_dir.starts_with(source_common.join("worktrees")),
            "worktree belongs to a different repository"
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
        clean(&self.path)
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
        clean(&self.path)
    }
    /// Explicitly discard tracked changes in this verified worktree and restore
    /// a known checkpoint. Any untracked *or ignored* files cause refusal: Git
    /// reset may otherwise overwrite them. Never removes untracked files.
    pub fn reset_to(&self, checkpoint: &str) -> Result<()> {
        self.verify_owned()?;
        detached(&self.path)?;
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
