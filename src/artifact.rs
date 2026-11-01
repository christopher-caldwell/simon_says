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
pub fn tracked_blob(repo: &Path, commit: &str, path: &str) -> Result<Vec<u8>> {
    let output = git(repo, &["ls-tree", "-z", "--full-tree", commit, "--", path])?;
    let entries: Vec<_> = output
        .split(|b| *b == 0)
        .filter(|e| !e.is_empty())
        .collect();
    ensure!(
        entries.len() == 1,
        "required tracked file missing or ambiguous: {path}"
    );
    let entry = std::str::from_utf8(entries[0]).context("non-UTF8 Git tree entry")?;
    let (meta, actual) = entry.split_once('\t').context("invalid Git tree entry")?;
    let mut fields = meta.split_whitespace();
    let mode = fields.next().unwrap_or("");
    let kind = fields.next().unwrap_or("");
    let object = fields.next().unwrap_or("");
    ensure!(
        actual == path
            && kind == "blob"
            && matches!(mode, "100644" | "100755")
            && !object.is_empty(),
        "expected regular tracked file: {path}"
    );
    git(repo, &["cat-file", "blob", object])
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
