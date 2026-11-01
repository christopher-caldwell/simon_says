//! Conservative cleanup of completed-run detached review worktrees.
//!
//! No run records, logs, user source checkout, worker checkout, refs or Git
//! objects are removed. This is not a defense against hostile same-user code.
use crate::{git::Worktree, provider::InvocationPlan, routing::Status, store::RunStore};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let mut command = Command::new("git");
    command
        .args(["-c", "core.hooksPath=/dev/null", "-C"])
        .arg(repo)
        .args(args)
        .env("GIT_NO_REPLACE_OBJECTS", "1");
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
        command.env_remove(key);
    }
    let output = crate::git_process::run(&mut command, None).context("run git worktree cleanup")?;
    ensure!(
        output.status.success(),
        "git worktree cleanup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}
fn regular_file(path: &Path) -> Result<Vec<u8>> {
    let m = fs::symlink_metadata(path).with_context(|| format!("inspect {}", path.display()))?;
    ensure!(
        m.is_file() && !m.file_type().is_symlink(),
        "unsafe action record {}",
        path.display()
    );
    Ok(fs::read(path)?)
}
fn directory(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path).with_context(|| format!("inspect {}", path.display()))?;
    ensure!(
        m.is_dir() && !m.file_type().is_symlink() && fs::canonicalize(path)? == path,
        "unsafe review path {}",
        path.display()
    );
    Ok(())
}
fn inspect_review(root: &Path, worker: &Worktree, review_dir: &Path, id: &str) -> Result<Worktree> {
    directory(review_dir)?;
    let path = review_dir.join("worktree");
    directory(&path)?;
    let action_dir = root.join("actions").join(id);
    directory(&action_dir)?;
    let packet: Value = serde_json::from_slice(&regular_file(&action_dir.join("packet.json"))?)?;
    let plan: InvocationPlan =
        serde_json::from_slice(&regular_file(&action_dir.join("invocation.json"))?)?;
    ensure!(
        packet.get("action_id").and_then(Value::as_str) == Some(id),
        "review action identity mismatch"
    );
    ensure!(
        matches!(
            packet.get("gate").and_then(Value::as_str),
            Some("reviewer" | "unblocker")
        ),
        "review action gate mismatch"
    );
    ensure!(plan.cwd == path, "review invocation checkout mismatch");
    let commit = packet
        .get("candidate_commit")
        .and_then(Value::as_str)
        .or_else(|| packet.get("baseline_commit").and_then(Value::as_str))
        .context("review checkpoint missing")?;
    let review: Worktree = serde_json::from_value(packet.get("review_worktree").cloned().context(
        "legacy review packet lacks recorded worktree identity; manual reconciliation required",
    )?)
    .context("invalid recorded review worktree identity")?;
    ensure!(
        review.source == worker.source
            && review.run_dir == review_dir
            && review.path == path
            && review.baseline == commit,
        "recorded review worktree does not match action identity"
    );
    review
        .verify_candidate(commit, commit)
        .context("review checkout is not its recorded clean candidate")?;
    review.verify_pristine()?;
    Ok(review)
}
/// Remove only clean, registered detached Reviewer/Unblocker worktrees from a
/// completed run. The worker worktree, all action evidence and run state are
/// retained. A missing, changed, dirty, ignored or unidentifiable checkout
/// causes refusal. Re-running after a partial cleanup skips already removed
/// worktrees. The caller must ensure that no action descendants remain alive.
pub fn cleanup_reviews(run_dir: &Path) -> Result<usize> {
    let store = RunStore::open(run_dir)?;
    let (record, _) = store.load()?;
    ensure!(
        record.state.status == Status::Complete,
        "review cleanup requires a completed run"
    );
    let worker: Worktree = serde_json::from_value(record.worktree)?;
    ensure!(
        worker.run_dir == store.root(),
        "worker worktree does not belong to run"
    );
    worker.verify_candidate(&record.state.accepted_commit, &record.state.accepted_commit)?;
    let reviews = store.root().join("reviews");
    directory(&reviews)?;
    let mut entries: Vec<_> = fs::read_dir(&reviews)?.collect::<std::result::Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    let mut targets = Vec::new();
    for entry in entries {
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-UTF8 review directory"))?;
        ensure!(
            id.strip_prefix("action-").is_some_and(
                |suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
            ),
            "unrecognized review directory {id:?}"
        );
        let dir = reviews.join(&id);
        directory(&dir)?;
        let worktree = dir.join("worktree");
        if fs::symlink_metadata(&worktree).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            continue;
        }
        targets.push((
            dir.clone(),
            id.clone(),
            inspect_review(store.root(), &worker, &dir, &id)?,
        ));
    }
    let mut count = 0;
    for (dir, id, review) in targets {
        store.verify_lock()?;
        let checked = inspect_review(store.root(), &worker, &dir, &id)?;
        ensure!(
            checked.git_dir == review.git_dir
                && checked.baseline == review.baseline
                && checked.git_identity == review.git_identity
                && checked.checkout_identity == review.checkout_identity
                && checked.source_identity == review.source_identity
                && checked.common_identity == review.common_identity,
            "review identity changed during cleanup"
        );
        let path = checked.path.to_str().context("review path is not UTF-8")?;
        git(&worker.source, &["worktree", "remove", "--", path])?;
        count += 1;
    }
    Ok(count)
}
