//! Validate regular tracked files directly from a commit, never the mutable checkout.
use crate::{Snapshot, Stage};
use anyhow::{bail, ensure, Context, Result};
use std::{path::Path, process::Command};

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(args);
    command.env("GIT_NO_REPLACE_OBJECTS", "1");
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
    let output = command.output().context("start git")?;
    ensure!(
        output.status.success(),
        "git {} failed: {}",
        args.first().unwrap_or(&""),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}
/// The identity of a regular tracked file, as stored in a specific Git tree.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BlobIdentity {
    mode: String,
    oid: String,
}

fn regular_blob_identity(repo: &Path, commit: &str, path: &str) -> Result<BlobIdentity> {
    crate::validate_relative_path(path)
        .with_context(|| format!("invalid tracked path {path:?}"))?;
    // Git pathspecs interpret punctuation and wildcards by default; force a
    // literal path and require the returned name to match the requested bytes.
    let literal = format!(":(literal){path}");
    let output = git(
        repo,
        &["ls-tree", "-z", "--full-tree", commit, "--", &literal],
    )?;
    let entries: Vec<_> = output
        .split(|b| *b == 0)
        .filter(|e| !e.is_empty())
        .collect();
    ensure!(
        entries.len() == 1,
        "required tracked file missing or ambiguous: {path}"
    );
    let separator = entries[0]
        .iter()
        .position(|b| *b == b'\t')
        .context("invalid Git tree entry")?;
    let metadata =
        std::str::from_utf8(&entries[0][..separator]).context("non-UTF8 Git tree metadata")?;
    let actual = &entries[0][separator + 1..];
    let mut fields = metadata.split_whitespace();
    let mode = fields.next().unwrap_or("");
    let kind = fields.next().unwrap_or("");
    let oid = fields.next().unwrap_or("");
    ensure!(
        actual == path.as_bytes()
            && kind == "blob"
            && matches!(mode, "100644" | "100755")
            && (oid.len() == 40 || oid.len() == 64)
            && oid
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            && fields.next().is_none(),
        "expected regular tracked file: {path}"
    );
    Ok(BlobIdentity {
        mode: mode.to_owned(),
        oid: oid.to_owned(),
    })
}

fn exact_commit(repo: &Path, commit: &str) -> Result<()> {
    ensure!(
        (commit.len() == 40 || commit.len() == 64)
            && commit
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "expected a full lowercase Git commit ID"
    );
    let actual = git(
        repo,
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    ensure!(
        std::str::from_utf8(&actual)?.trim() == commit,
        "Git ID does not resolve to the exact commit"
    );
    Ok(())
}

/// Ensure every declared authority path is a regular tracked file with the
/// same Git blob and mode at both the initial baseline and candidate commit.
/// This checks Git identity, not the mutable filesystem or external effects.
pub fn frozen_paths(repo: &Path, baseline: &str, candidate: &str, paths: &[String]) -> Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    exact_commit(repo, baseline).context("invalid authority baseline")?;
    exact_commit(repo, candidate).context("invalid authority candidate")?;
    for path in paths {
        let original = regular_blob_identity(repo, baseline, path)
            .with_context(|| format!("authority baseline {path:?}"))?;
        let proposed = regular_blob_identity(repo, candidate, path)
            .with_context(|| format!("authority candidate {path:?}"))?;
        ensure!(
            original == proposed,
            "frozen authority file changed: {path}"
        );
    }
    Ok(())
}

pub fn tracked_blob(repo: &Path, commit: &str, path: &str) -> Result<Vec<u8>> {
    let identity = regular_blob_identity(repo, commit, path)?;
    git(repo, &["cat-file", "blob", &identity.oid])
}

pub fn inputs(repo: &Path, commit: &str, stage: &Stage) -> Result<()> {
    for path in &stage.inputs {
        tracked_blob(repo, commit, path).with_context(|| format!("input {path}"))?;
    }
    Ok(())
}
pub fn outputs(repo: &Path, commit: &str, stage: &Stage, snapshot: &Snapshot) -> Result<()> {
    for path in &stage.outputs {
        let bytes = tracked_blob(repo, commit, path).with_context(|| format!("output {path}"))?;
        if let Some(schema_path) = &stage.output_schema {
            let schema_text = snapshot
                .files
                .get(schema_path)
                .context("frozen schema missing")?;
            let schema: serde_json::Value = serde_json::from_str(schema_text)?;
            let compiled = jsonschema::JSONSchema::options()
                .with_draft(jsonschema::Draft::Draft7)
                .compile(&schema)
                .map_err(|e| anyhow::anyhow!("schema: {e}"))?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .with_context(|| format!("output {path} is not JSON"))?;
            if let Err(errors) = compiled.validate(&value) {
                let messages: Vec<_> = errors.take(10).map(|e| e.to_string()).collect();
                bail!("output {path} violates schema: {}", messages.join("; "));
            };
        }
    }
    Ok(())
}
