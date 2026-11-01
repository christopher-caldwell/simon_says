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
    /// The exact provider prompt, durably recorded in the invocation before launch.
    /// Native adapters receive it via a regular file on stdin, avoiding argv limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin_prompt: Option<String>,
    pub cwd: PathBuf,
    pub packet_path: PathBuf,
    pub adapter: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    /// The exact previously observed Codex session, when a same-stage Worker
    /// correction opted in to continuation. Never inferred from --last.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_in: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub scratch: Option<PathBuf>,
}

fn default_timeout() -> u64 {
    1800
}

// Provider event streams can contain tool output. Bound each durable log so a
// runaway action cannot make the controller allocate an unbounded response.
const MAX_ACTION_LOG_BYTES: u64 = 64 * 1024 * 1024;

fn check_log_sizes(stdout: &Path, stderr: &Path) -> Result<()> {
    for path in [stdout, stderr] {
        ensure!(
            fs::metadata(path)?.len() <= MAX_ACTION_LOG_BYTES,
            "action log {} exceeded the 64 MiB limit; action is uncertain",
            path.display()
        );
    }
    Ok(())
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
            "exec",
            "resume",
            "--json",
            "-C",
            "--cd",
            "--model",
            "-m",
            "-c",
            "--config",
            "--",
            // Session selection and checkout identity are controller-owned.
            "--last",
            "--all",
            "--worktree",
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
    prepare_with_session(role, gate, prompt, cwd, packet_path, None)
}

/// Prepare an explicitly opted-in, same-stage Codex Worker continuation.
/// The caller owns session scope and must never recover a handle after restart.
pub fn prepare_with_session(
    role: &Role,
    gate: Gate,
    prompt: &str,
    cwd: &Path,
    packet_path: &Path,
    session_in: Option<&str>,
) -> Result<InvocationPlan> {
    if role.resume_worker_session {
        ensure!(
            role.kind == "agent" && role.adapter.as_deref() == Some("codex"),
            "resume_worker_session currently requires a Codex agent"
        );
        ensure!(
            !role.args.iter().any(
                |arg| arg.split_once('=').map_or(arg.as_str(), |(flag, _)| flag) == "--ephemeral"
            ),
            "resume_worker_session is incompatible with --ephemeral"
        );
    }
    if let Some(id) = session_in {
        ensure!(
            gate == Gate::Worker
                && role.resume_worker_session
                && role.kind == "agent"
                && role.adapter.as_deref() == Some("codex"),
            "session continuation is permitted only for opted-in Codex Workers"
        );
        let parsed = uuid::Uuid::parse_str(id).context("invalid Codex session identity")?;
        ensure!(
            parsed.to_string() == id,
            "Codex session identity must be a canonical UUID"
        );
    }
    ensure!(
        (1..=86400).contains(&role.timeout_seconds.unwrap_or_else(default_timeout)),
        "timeout_seconds must be 1..=86400"
    );
    if role.kind == "agent" {
        ensure!(
            prompt.len() <= 4 * 1024 * 1024,
            "native-provider prompt exceeds the 4 MiB limit"
        );
    }
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
                    if let Some(id) = session_in {
                        argv.extend(["resume".into(), id.to_owned()]);
                    }
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
                    // Native Codex `exec -` reads the authoritative prompt from stdin.
                    argv.push("-".into());
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
                    // As in Orchestrate, retain `--` so variadic native args cannot
                    // consume a positional prompt; Claude reads stdin when omitted.
                    argv.push("--".into());
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
        stdin_prompt: (role.kind == "agent").then(|| prompt.to_owned()),
        cwd,
        packet_path: packet_path.to_owned(),
        adapter,
        model: role.model.clone(),
        effort: role.effort.clone(),
        session_in: session_in.map(str::to_owned),
        timeout_seconds: role.timeout_seconds.unwrap_or_else(default_timeout),
        scratch: None,
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
    // A seekable file avoids a blocking pipe writer and gives the provider a
    // durable, inspectable copy of the exact prompt. The serialized plan was
    // persisted before Running; a crash at any point is still uncertain.
    let stdin = if let Some(prompt) = &plan.stdin_prompt {
        use std::io::{Seek, Write};
        ensure!(
            prompt.len() <= 4 * 1024 * 1024,
            "native-provider prompt exceeds the 4 MiB limit"
        );
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let path = action_dir.join("prompt.txt");
        let mut file = opts
            .open(&path)
            .with_context(|| format!("create {}", path.display()))?;
        file.write_all(prompt.as_bytes())?;
        file.sync_all()?;
        file.rewind()?;
        Stdio::from(file)
    } else {
        Stdio::null()
    };
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.argv)
        .current_dir(&plan.cwd)
        .env("SIMON_PACKET", &plan.packet_path)
        .stdin(stdin)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    if let Some(scratch) = &plan.scratch {
        command.env("SIMON_SCRATCH", scratch);
    }
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
    let receipt_result = (|| -> Result<()> {
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
        Ok(())
    })();
    if let Err(error) = receipt_result {
        let reaped = crate::git_process::terminate_unreaped(&mut child);
        return Err(error.context(format!("record action process; termination attempted (leader reaped: {reaped}); action and external effects remain uncertain")));
    }
    #[cfg(unix)]
    let group = child.id() as libc::pid_t;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(plan.timeout_seconds);
    let status = loop {
        if let Err(error) = check_log_sizes(&stdout_path, &stderr_path) {
            let reaped = crate::git_process::terminate_unreaped(&mut child);
            let message = format!("{error}; process-group termination attempted (leader reaped: {reaped}); processes and external effects remain uncertain");
            return Err(error.context(message));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "wait for {}; processes and external effects remain uncertain",
                        plan.program
                    )
                });
            }
        }
        if std::time::Instant::now() >= deadline {
            // The direct child is still alive and unreaped, so its PID cannot be reused.
            // Stop the action's own process group, then retain Running/uncertain state.
            let reaped = crate::git_process::terminate_unreaped(&mut child);
            let _ = fs::File::open(&stdout_path).and_then(|f| f.sync_all());
            let _ = fs::File::open(&stderr_path).and_then(|f| f.sync_all());
            bail!("action timed out after {} seconds; process-group termination attempted (leader reaped: {reaped}); processes and external effects remain uncertain", plan.timeout_seconds);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
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
    if let Some(prompt) = &plan.stdin_prompt {
        let path = action_dir.join("prompt.txt");
        let meta = fs::symlink_metadata(&path)?;
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "provider prompt record changed type"
        );
        ensure!(
            meta.len() == prompt.len() as u64,
            "provider prompt record changed size"
        );
        use std::io::Read;
        let mut opts = OpenOptions::new();
        opts.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.custom_flags(libc::O_NOFOLLOW);
        }
        let mut bytes = Vec::with_capacity(prompt.len());
        opts.open(&path)?
            .take(prompt.len() as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes == prompt.as_bytes(),
            "provider prompt record changed during execution"
        );
    }
    fs::File::open(&stdout_path)?.sync_all()?;
    fs::File::open(&stderr_path)?.sync_all()?;
    check_log_sizes(&stdout_path, &stderr_path)?;
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(&stdout_path)
        .with_context(|| format!("read {}", stdout_path.display()))?
        .take(MAX_ACTION_LOG_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_ACTION_LOG_BYTES,
        "action stdout exceeded the 64 MiB limit; action is uncertain"
    );
    let text = String::from_utf8(bytes).context("provider stdout is not UTF-8")?;
    if plan.adapter != "command" {
        ensure!(
            !native_terminal_failure(&text, &plan.adapter),
            "native provider reported terminal failure; action and external effects remain uncertain"
        );
        ensure!(
            native_terminal_success(&text, &plan.adapter),
            "native provider omitted a terminal success event; action and external effects remain uncertain"
        );
    }
    let (response, session_id) = if plan.adapter == "command" {
        (Some(text), None)
    } else {
        (
            extract_native_text(&text, &plan.adapter),
            extract_response(&text).1,
        )
    };
    Ok(InvocationOutcome {
        success: status.success(),
        exit_code: status.code(),
        response,
        session_id,
    })
}

fn extract_native_text(stdout: &str, adapter: &str) -> Option<String> {
    let allowed = |value: &Value| match adapter {
        "codex" => {
            value.get("type").and_then(Value::as_str) == Some("item.completed")
                && value
                    .get("item")
                    .and_then(|v| v.get("type"))
                    .and_then(Value::as_str)
                    == Some("agent_message")
        }
        "claude" => {
            value.get("type").and_then(Value::as_str) == Some("result")
                && value.get("subtype").and_then(Value::as_str) == Some("success")
                && value.get("is_error").and_then(Value::as_bool) == Some(false)
        }
        _ => false,
    };
    let mut response = None;
    for value in stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        if adapter == "codex" && value.get("type").and_then(Value::as_str) == Some("turn.started") {
            response = None;
        }
        if allowed(&value) {
            response = final_text(&value);
        }
    }
    response.or_else(|| {
        serde_json::from_str::<Value>(stdout)
            .ok()
            .filter(&allowed)
            .and_then(|v| final_text(&v))
    })
}

fn native_terminal_success(stdout: &str, adapter: &str) -> bool {
    let matches = |value: &Value| {
        let kind = value.get("type").and_then(Value::as_str);
        match adapter {
            "codex" => kind == Some("turn.completed"),
            "claude" => {
                kind == Some("result")
                    && value.get("subtype").and_then(Value::as_str) == Some("success")
                    && value.get("is_error").and_then(Value::as_bool) == Some(false)
            }
            _ => false,
        }
    };
    let mut terminal_after_response = false;
    for value in stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        if final_text(&value).is_some()
            || (adapter == "codex"
                && value.get("type").and_then(Value::as_str) == Some("turn.started"))
        {
            terminal_after_response = false;
        }
        if matches(&value) {
            terminal_after_response = true;
        }
    }
    terminal_after_response
        || serde_json::from_str::<Value>(stdout)
            .ok()
            .is_some_and(|v| matches(&v))
}

/// Process exit success alone cannot override an explicit failed native turn.
/// Only terminal events are considered: progress warnings may be recoverable.
fn native_terminal_failure(stdout: &str, adapter: &str) -> bool {
    fn failed(value: &Value, adapter: &str) -> bool {
        let kind = value.get("type").and_then(Value::as_str);
        match adapter {
            "codex" => kind == Some("turn.failed"),
            "claude" if kind == Some("result") => {
                value.get("is_error").and_then(Value::as_bool) != Some(false)
                    || value.get("subtype").and_then(Value::as_str) != Some("success")
            }
            _ => false,
        }
    }
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .any(|v| failed(&v, adapter))
        || serde_json::from_str::<Value>(stdout)
            .ok()
            .is_some_and(|v| failed(&v, adapter))
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

/// Claude Worker policy may require its own Bash sandbox, including via merged
/// user settings. Do not guess its effective permissions or disable a sandbox.
pub fn requires_nested_sandbox_probe(role: &Role, gate: Gate) -> bool {
    role.kind == "agent" && role.adapter.as_deref() == Some("claude") && gate == Gate::Worker
}
