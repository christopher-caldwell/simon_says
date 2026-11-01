//! macOS action write restriction for run authority.
//!
//! This profile guards ordinary direct filesystem writes; it is not a hostile-code
//! security boundary. An action may use network or IPC, can alter other files owned
//! by the user, and can modify a linked worktree's shared Git repository. The caller
//! must persist the returned *wrapped* argv before spawning and fail closed if setup
//! or launch fails. On other operating systems this implementation fails closed.
use anyhow::Result;
#[cfg(target_os = "macos")]
use anyhow::{ensure, Context};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxPlan {
    pub program: String,
    pub argv: Vec<String>,
    pub scratch: PathBuf,
}

#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
fn canonical_exact(path: &Path) -> Result<PathBuf> {
    let canonical =
        std::fs::canonicalize(path).with_context(|| format!("canonicalize {}", path.display()))?;
    ensure!(
        canonical == path,
        "sandbox paths must be absolute, canonical and free of symlink components: {}",
        path.display()
    );
    ensure!(
        canonical.is_dir(),
        "sandbox path is not a directory: {}",
        path.display()
    );
    Ok(canonical)
}
#[cfg(target_os = "macos")]
#[cfg(target_os = "macos")]
fn literal(path: &Path) -> Result<String> {
    let s = path.to_str().context("sandbox paths must be UTF-8")?;
    ensure!(
        !s.chars().any(|c| c.is_control()),
        "sandbox path contains control characters"
    );
    Ok(format!(
        "\"{}\"",
        s.replace('\\', "\\\\").replace('"', "\\\"")
    ))
}

/// Construct an invocation using `/usr/bin/sandbox-exec` on macOS.
///
/// `run` is the canonical run root; `cwd` is its worker checkout or this action's
/// detached review checkout; `scratch` is a freshly created directory at
/// `run/actions/<action-id>/scratch`. The sandbox blocks writes to the rest of
/// `run` while permitting the checkout and scratch. The caller is responsible for
/// setting `SIMON_SCRATCH`, opening transport logs before launch, and preserving
/// the normal Git/state checks. `scratch` should not contain authority records.
pub fn prepare(
    run: &Path,
    cwd: &Path,
    scratch: &Path,
    program: &str,
    args: &[String],
) -> Result<SandboxPlan> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (run, cwd, scratch, program, args);
        anyhow::bail!("action sandbox is only supported on macOS");
    }
    #[cfg(target_os = "macos")]
    {
        ensure!(
            !program.is_empty() && !program.contains('\0'),
            "invalid action program"
        );
        let run = canonical_exact(run)?;
        let cwd = canonical_exact(cwd)?;
        let scratch = canonical_exact(scratch)?;
        ensure!(
            scratch.file_name().is_some_and(|n| n == "scratch"),
            "scratch must be named scratch"
        );
        let action_dir = scratch
            .parent()
            .context("scratch has no action directory")?;
        ensure!(
            action_dir.parent() == Some(run.join("actions").as_path()),
            "scratch must belong to this run's actions directory"
        );
        let id = action_dir
            .file_name()
            .and_then(|s| s.to_str())
            .context("invalid action id")?;
        ensure!(
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "invalid action id"
        );
        ensure!(
            cwd == run.join("worktree") || cwd == run.join("reviews").join(id).join("worktree"),
            "checkout must be this action's controlled worktree"
        );
        let profile = format!("(version 1)\n(allow default)\n(deny file-write* (subpath {}))\n(allow file-write* (subpath {}) (subpath {}))\n", literal(&run)?, literal(&cwd)?, literal(&scratch)?);
        let binary = Path::new("/usr/bin/sandbox-exec");
        ensure!(binary.is_file(), "macOS sandbox-exec is unavailable");
        // Probe syntax and availability before offering a plan. This does not
        // prove policy coverage for arbitrary subprocesses or IPC.
        let status = std::process::Command::new(binary)
            .args(["-p", &profile, "--", "/usr/bin/true"])
            .status()
            .context("probe macOS action sandbox")?;
        ensure!(status.success(), "macOS action sandbox probe failed");
        let mut argv = vec!["-p".into(), profile, "--".into(), program.into()];
        argv.extend_from_slice(args);
        Ok(SandboxPlan {
            program: binary.to_string_lossy().into_owned(),
            argv,
            scratch,
        })
    }
}
