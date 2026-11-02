//! Explicit recovery of a completed Worker response after mechanical checking stopped.
//! No checkout content is moved or removed here; operators must preserve and
//! reconcile unexpected files separately before attempting adoption.
use crate::{
    artifact,
    git::Worktree,
    provider,
    routing::{Gate, Response, Status},
    store::{recovery_commit, RunRecord, RunStore},
    Workflow,
};
use anyhow::{ensure, Context, Result};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const LIMIT: u64 = 64 * 1024 * 1024;
const IGNORED_STOP: &str =
    "managed worktree contains ignored or untracked files; preserve them and reconcile explicitly";

fn directory(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "unsafe recovery directory {}",
        path.display()
    );
    Ok(())
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    ensure!(
        before.is_file() && !before.file_type().is_symlink() && before.len() <= limit,
        "unsafe or oversized recovery record {}",
        path.display()
    );
    let mut opts = fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    let file = opts.open(path)?;
    let opened = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            before.dev() == opened.dev() && before.ino() == opened.ino() && opened.nlink() == 1,
            "recovery record changed during open"
        );
    }
    ensure!(
        opened.is_file() && opened.len() <= limit,
        "unsafe recovery record"
    );
    let mut data = Vec::new();
    file.take(limit + 1).read_to_end(&mut data)?;
    ensure!(data.len() as u64 <= limit, "oversized recovery record");
    Ok(data)
}
fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&read(path, LIMIT)?)?)
}

/// Adopt an already recorded complete Worker, never replaying it. The exact
/// expected action and candidate are operator-supplied to prevent stale recovery.
pub fn adopt(
    run_dir: &Path,
    expected_action: &str,
    expected_candidate: &str,
    acknowledge: bool,
    reconcile_check: bool,
) -> Result<RunRecord> {
    ensure!(acknowledge, "adoption requires --acknowledge-external-effects after confirming processes exited and reconciling external effects");
    let store = RunStore::open(run_dir)?;
    let (mut record, snapshot) = store.load()?;
    ensure!(
        ((record.state.status == Status::Running && !reconcile_check)
            || (record.state.status == Status::AdoptionChecking && reconcile_check))
            && record.state.gate == Gate::Worker
            && record.state.action_id.as_deref() == Some(expected_action),
        "run is not the expected running Worker action"
    );
    ensure!(
        record.state.stop_reason.as_deref() == Some(IGNORED_STOP),
        "adoption requires the recorded ignored/untracked checkout stop"
    );
    let number = expected_action
        .strip_prefix("action-")
        .context("invalid action identity")?
        .parse::<u64>()?;
    ensure!(
        expected_action == format!("action-{number}")
            && number.checked_add(1) == Some(record.next_action),
        "action counter mismatch"
    );
    let workflow: Workflow = toml::from_str(&snapshot.template)?;
    let stage = workflow
        .stages
        .get(&record.state.stage)
        .context("unknown stage")?;
    let role = workflow
        .roles
        .get(&stage.worker)
        .context("unknown Worker role")?;
    let worktree: Worktree = serde_json::from_value(record.worktree.clone())?;
    ensure!(
        worktree.run_dir == store.root() && worktree.path == store.root().join("worktree"),
        "worktree ownership mismatch"
    );
    let actions = store.root().join("actions");
    directory(&actions)?;
    let action = actions.join(expected_action);
    directory(&action)?;
    let scratch = action.join("scratch");
    directory(&scratch)?;
    let packet_path = action.join("packet.json");
    let packet: Value = read_json(&packet_path)?;
    let plan: provider::InvocationPlan = read_json(&action.join("invocation.json"))?;
    ensure!(plan.session_in.is_none(), "adoption of a resumed Worker requires independently verified session provenance and is not supported");
    let outcome: provider::InvocationOutcome = read_json(&action.join("outcome.json"))?;
    let failure: Value = read_json(&action.join("failure.json"))?;
    ensure!(
        failure["error"] == IGNORED_STOP,
        "failure record does not match recoverable stop"
    );
    let mut env = BTreeMap::<String, PathBuf>::new();
    for (name, relative) in &role.scratch_env {
        let mut target = scratch.clone();
        for part in Path::new(relative).components() {
            target.push(part.as_os_str());
            directory(&target)?;
        }
        ensure!(
            target.canonicalize()? == target,
            "unsafe scratch environment path"
        );
        env.insert(name.clone(), target);
    }
    let instructions = snapshot
        .files
        .get(&stage.instructions)
        .context("frozen instructions missing")?;
    let profile = role
        .profile
        .as_ref()
        .map(|p| snapshot.files.get(p).context("frozen profile missing"))
        .transpose()?;
    let mut expected_packet = json!({"schema_version":1, "run_id":record.run_id, "action_id":expected_action, "template_digest":snapshot.sha256, "workflow_purpose":workflow.purpose, "run_context":snapshot.run_context, "stage":record.state.stage, "gate":Gate::Worker, "role":stage.worker, "baseline_commit":record.state.accepted_commit, "candidate_commit":record.state.candidate_commit, "inputs":stage.inputs, "protected_inputs":stage.protected_inputs, "outputs":stage.outputs, "output_schema":stage.output_schema.as_ref().and_then(|p| snapshot.files.get(p)), "instructions":instructions, "profile":profile, "feedback":record.state.feedback, "unblock_origin":record.state.unblock_origin, "review_routes":null, "last_transition":record.state.last_transition, "review_worktree":null});
    if stage.route_selector.as_deref() == Some("worker") {
        expected_packet["worker_routes"] = json!(stage.routes.keys().collect::<Vec<_>>());
    }
    if packet.get("scratch_env").is_some() {
        expected_packet["scratch_env"] = serde_json::to_value(&env)?;
    } else {
        ensure!(
            env.is_empty(),
            "legacy packet cannot omit configured scratch environment"
        );
    }
    ensure!(
        packet == expected_packet,
        "recorded action packet differs from frozen Worker authority"
    );
    let prompt = crate::controller::action_prompt(&packet)?;
    let mut expected_plan = provider::prepare_with_session(
        role,
        Gate::Worker,
        &prompt,
        &worktree.path,
        &packet_path,
        plan.session_in.as_deref(),
    )?;
    ensure!(plan.program != "/usr/bin/sandbox-exec", "legacy sandbox-wrapped Worker records cannot be adopted by this version; preserve and reconcile the run explicitly");
    // Git verification can invoke external processes or filters. Persist uncertainty
    // before it starts. An interrupted or
    // failed check cannot be retried without an explicit reconciliation flag.
    if record.state.status == Status::Running {
        record.state.status = Status::AdoptionChecking;
        store.save(&record)?;
    }
    expected_plan.scratch = Some(scratch);
    expected_plan.env = env;
    ensure!(
        serde_json::to_value(&plan)? == serde_json::to_value(&expected_plan)?,
        "recorded invocation differs from frozen Worker plan"
    );
    let process: Value = read_json(&action.join("process.json"))?;
    let pid = process["pid"]
        .as_u64()
        .context("process identity missing")?;
    ensure!(
        pid > 1 && pid <= i32::MAX as u64 && process["process_group"].as_u64() == Some(pid),
        "invalid process group receipt"
    );
    #[cfg(unix)]
    {
        let rc = unsafe { libc::kill(-(pid as libc::pid_t), 0) };
        let error = std::io::Error::last_os_error();
        ensure!(
            rc != 0 && error.raw_os_error() == Some(libc::ESRCH),
            "recorded process group may still exist; reconcile before adoption"
        );
    }
    let stdout = String::from_utf8(read(&action.join("stdout.txt"), LIMIT)?)?;
    let _stderr = read(&action.join("stderr.txt"), LIMIT)?;
    if let Some(prompt) = &plan.stdin_prompt {
        ensure!(
            read(&action.join("prompt.txt"), 4 * 1024 * 1024)? == prompt.as_bytes(),
            "persisted prompt differs from invocation"
        );
    }
    provider::validate_recorded_outcome(&plan, &stdout, &outcome)?;
    let response: Response = serde_json::from_str(
        outcome
            .response
            .as_deref()
            .context("Worker response missing")?,
    )?;
    ensure!(
        response.action_id == expected_action
            && response.outcome == "complete"
            && response.candidate_commit.as_deref() == Some(expected_candidate),
        "response does not nominate expected Worker candidate"
    );
    let checkpoint = recovery_commit(&record.state).to_owned();
    worktree.verify_candidate(expected_candidate, &checkpoint)?;
    artifact::inputs(&worktree.path, &checkpoint, stage)?;
    artifact::frozen_paths(
        &worktree.path,
        &worktree.baseline,
        expected_candidate,
        &workflow.frozen_project_files,
    )?;
    artifact::frozen_paths(
        &worktree.path,
        &checkpoint,
        expected_candidate,
        &stage.protected_inputs,
    )?;
    artifact::outputs(&worktree.path, expected_candidate, stage, &snapshot)?;
    record.state.status = Status::Running;
    record.state.stop_reason = None;
    record.state.apply(&workflow, response)?;
    ensure!(
        record.state.status == Status::Ready && record.state.gate == Gate::Reviewer,
        "adoption did not lead to independent review"
    );
    store.save(&record)?;
    Ok(record)
}
