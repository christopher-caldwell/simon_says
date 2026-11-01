//! Native provider and deterministic command subprocess adapter.
//!
//! Adapted from Orchestrate's `crates/core/src/provider.rs` and Build adapter.
//! The caller must persist the complete plan and mark the action running before
//! invoking it, and must not replay an uncertain result automatically.
use crate::{routing::Gate, Role};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationPlan {
    pub program: String,
    /// Does not include `program`; arguments are passed directly, never through a shell.
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub packet_path: PathBuf,
    pub adapter: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationOutcome {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub response: Option<String>,
    pub session_id: Option<String>,
}

/// Adapter-controlled flags cannot be replaced through the template's native arguments.
pub fn validate_native_args(adapter: &str, args: &[String]) -> Result<()> {
    let owned: &[&str] = match adapter {
        "codex" => &[
            "exec", "resume", "--json", "-C", "--cd", "--model", "-m", "-c", "--config", "--",
        ],
        "claude" => &[
            "--",
            "--print",
            "-p",
            "--verbose",
            "--output-format",
            "--resume",
            "--model",
            "--effort",
        ],
        _ => bail!("unsupported provider adapter {adapter:?}"),
    };
    for arg in args {
        ensure!(!arg.is_empty(), "native arguments cannot be empty");
        let flag = arg.split_once('=').map_or(arg.as_str(), |(flag, _)| flag);
        ensure!(
            !owned.contains(&flag),
            "native argument {arg:?} collides with adapter-owned invocation flags"
        );
        if adapter == "codex" {
            ensure!(
                !flag.starts_with("-C") && !flag.starts_with("-c") && !flag.starts_with("-m"),
                "native argument {arg:?} collides with adapter-owned invocation flags"
            );
        }
    }
    Ok(())
}

/// Creates the exact argv/environment receipt. The caller should write it durably
/// before launching and include the prompt and packet in durable action records.
pub fn prepare(
    role: &Role,
    gate: Gate,
    prompt: &str,
    cwd: &Path,
    packet_path: &Path,
) -> Result<InvocationPlan> {
    let cwd = cwd
        .canonicalize()
        .with_context(|| format!("canonicalize invocation cwd {}", cwd.display()))?;
    ensure!(cwd.is_dir(), "invocation cwd is not a directory");
    ensure!(packet_path.is_absolute(), "packet path must be absolute");
    let (program, argv, adapter) = match role.kind.as_str() {
        "command" => {
            ensure!(
                !role.argv.is_empty() && !role.argv[0].is_empty(),
                "empty command argv"
            );
            (
                role.argv[0].clone(),
                role.argv[1..].to_vec(),
                "command".to_string(),
            )
        }
        "agent" => {
            let adapter = role.adapter.as_deref().context("agent adapter missing")?;
            validate_native_args(adapter, &role.args)?;
            let mut argv = Vec::new();
            let program = match adapter {
                "codex" => {
                    argv.extend([
                        "exec".into(),
                        "--json".into(),
                        "-C".into(),
                        cwd.to_string_lossy().into_owned(),
                    ]);
                    if let Some(model) = &role.model {
                        argv.extend(["--model".into(), model.clone()]);
                    }
                    if let Some(effort) = &role.effort {
                        ensure!(
                            matches!(
                                effort.as_str(),
                                "none"
                                    | "minimal"
                                    | "low"
                                    | "medium"
                                    | "high"
                                    | "xhigh"
                                    | "max"
                                    | "ultra"
                            ),
                            "unsupported Codex effort {effort:?}"
                        );
                        argv.extend(["-c".into(), format!("model_reasoning_effort=\"{effort}\"")]);
                    }
                    argv.extend(role.args.iter().cloned());
                    argv.push(prompt.into());
                    "codex"
                }
                "claude" => {
                    argv.extend([
                        "-p".into(),
                        "--verbose".into(),
                        "--output-format".into(),
                        "stream-json".into(),
                    ]);
                    if let Some(model) = &role.model {
                        argv.extend(["--model".into(), model.clone()]);
                    }
                    if let Some(effort) = &role.effort {
                        ensure!(
                            matches!(
                                effort.as_str(),
                                "low" | "medium" | "high" | "xhigh" | "max" | "ultracode"
                            ),
                            "unsupported Claude effort {effort:?}"
                        );
                        argv.extend(["--effort".into(), effort.clone()]);
                    }
                    // Match Build's worker default. Explicit permission policy is trusted configuration.
                    if gate == Gate::Worker && !has_claude_permission_policy(&role.args) {
                        argv.extend([
                            "--permission-mode".into(),
                            "acceptEdits".into(),
                            "--permission-prompts".into(),
                            "none".into(),
                            "--settings".into(),
                            CLAUDE_WORKER_SANDBOX_SETTINGS.into(),
                        ]);
                    }
                    argv.extend(role.args.iter().cloned());
                    argv.push("--".into());
                    argv.push(prompt.into());
                    "claude"
                }
                _ => bail!("unsupported provider adapter {adapter:?}"),
            };
            (program.to_owned(), argv, adapter.to_owned())
        }
        other => bail!("unsupported role kind {other:?}"),
    };
    Ok(InvocationPlan {
        program,
        argv,
        cwd,
        packet_path: packet_path.to_owned(),
        adapter,
        model: role.model.clone(),
        effort: role.effort.clone(),
    })
}

const CLAUDE_WORKER_SANDBOX_SETTINGS: &str = r#"{"sandbox":{"enabled":true,"autoAllowBashIfSandboxed":true,"allowUnsandboxedCommands":false,"failIfUnavailable":true}}"#;
fn has_claude_permission_policy(args: &[String]) -> bool {
    args.iter().any(|arg| {
        matches!(
            arg.split_once('=').map_or(arg.as_str(), |(flag, _)| flag),
            "--permission-mode"
                | "--allowedTools"
                | "--allowed-tools"
                | "--dangerously-skip-permissions"
                | "--allow-dangerously-skip-permissions"
                | "--permission-prompts"
                | "--permission-prompt-tool"
                | "--settings"
                | "--setting-sources"
        )
    })
}

/// Execute a prepared plan. The command receives SIMON_PACKET as an absolute
/// path. Transport logs are exclusively created to prevent accidental reuse.
/// A crash or error is uncertain; this function does not retry or clean up.
pub fn invoke(plan: &InvocationPlan, action_dir: &Path) -> Result<InvocationOutcome> {
    let stdout_path = action_dir.join("stdout.txt");
    let stderr_path = action_dir.join("stderr.txt");
    let open = |path: &Path| {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| format!("create {}", path.display()))
    };
    let stdout = open(&stdout_path)?;
    let stderr = open(&stderr_path)?;
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.argv)
        .current_dir(&plan.cwd)
        .env("SIMON_PACKET", &plan.packet_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    // Prevent inherited Git redirection from moving an action outside its checkout.
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
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .with_context(|| format!("invoke {}", plan.program))?;
    // Receipt is diagnostic only: a crash can occur between spawn and this write,
    // and PIDs can be reused. It never authorizes an automatic replay or reset.
    {
        use std::io::Write;
        let mut receipt = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(action_dir.join("process.json"))?;
        #[cfg(unix)]
        let process_group = Some(child.id());
        #[cfg(not(unix))]
        let process_group: Option<u32> = None;
        serde_json::to_writer_pretty(
            &mut receipt,
            &serde_json::json!({"pid": child.id(), "process_group": process_group}),
        )?;
        receipt.write_all(b"\n")?;
        receipt.sync_all()?;
    }
    #[cfg(unix)]
    let group = child.id() as libc::pid_t;
    let status = child
        .wait()
        .with_context(|| format!("wait for {}", plan.program))?;
    #[cfg(unix)]
    {
        // A surviving same-group descendant may still mutate work after the parent exits.
        let grace = if plan.adapter == "command" { 2 } else { 20 };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(grace);
        loop {
            let rc = unsafe { libc::kill(-group, 0) };
            let error = std::io::Error::last_os_error();
            if rc != 0 && error.raw_os_error() == Some(libc::ESRCH) {
                break;
            }
            ensure!(
                std::time::Instant::now() < deadline,
                "action left processes running in its process group; action is uncertain"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    fs::File::open(&stdout_path)?.sync_all()?;
    fs::File::open(&stderr_path)?.sync_all()?;
    let bytes =
        fs::read(&stdout_path).with_context(|| format!("read {}", stdout_path.display()))?;
    let text = String::from_utf8(bytes).context("provider stdout is not UTF-8")?;
    let (response, session_id) = if plan.adapter == "command" {
        (Some(text), None)
    } else {
        extract_response(&text)
    };
    Ok(InvocationOutcome {
        success: status.success(),
        exit_code: status.code(),
        response,
        session_id,
    })
}

/// Extract the final native agent message from the transport event stream.
pub fn extract_response(stdout: &str) -> (Option<String>, Option<String>) {
    let mut response = None;
    let mut session = None;
    for line in stdout.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if session.is_none() {
            session = value
                .get("thread_id")
                .or_else(|| value.get("session_id"))
                .or_else(|| value.get("conversation_id"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
        }
        if let Some(text) = final_text(&value) {
            response = Some(text);
        }
    }
    if response.is_none() {
        if let Ok(value) = serde_json::from_str::<Value>(stdout) {
            response = final_text(&value);
            session = session.or_else(|| {
                value
                    .get("thread_id")
                    .or_else(|| value.get("session_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        }
    }
    (response, session)
}
fn final_text(value: &Value) -> Option<String> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if matches!(
        event_type,
        "result" | "turn.completed" | "response.completed"
    ) {
        for key in ["result", "final_response", "text"] {
            if let Some(text) = value.get(key).and_then(Value::as_str) {
                return Some(text.to_owned());
            }
        }
    }
    if let Some(item) = value.get("item") {
        if item.get("type").and_then(Value::as_str) == Some("agent_message") {
            return content_text(item.get("content").or_else(|| item.get("text")));
        }
    }
    if let Some(message) = value.get("message") {
        if message.get("role").and_then(Value::as_str) == Some("assistant") {
            return content_text(message.get("content"));
        }
    }
    None
}
fn content_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(|item| {
                    (item.get("type").and_then(Value::as_str) == Some("text"))
                        .then(|| item.get("text").and_then(Value::as_str))
                        .flatten()
                })
                .collect::<Vec<_>>();
            (!parts.is_empty()).then(|| parts.join("\n"))
        }
        _ => None,
    }
}
