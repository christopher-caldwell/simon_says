//! Explicitly recover a recorded review that redundantly echoed its candidate.
//! The action is never replayed; both worktrees and all recorded authority are checked.
use crate::{
    adopt_worker::{directory, read, read_json, LIMIT},
    artifact,
    git::Worktree,
    provider,
    routing::{Gate, Response, Status},
    store::{RunRecord, RunStore},
    Workflow,
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

const LEGACY_STOP: &str = "reviewer cannot nominate candidate";

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
            && record.state.gate == Gate::Reviewer
            && record.state.action_id.as_deref() == Some(expected_action)
            && record.state.candidate_commit.as_deref() == Some(expected_candidate),
        "run is not the expected recorded Reviewer action and candidate"
    );
    ensure!(
        record.state.stop_reason.as_deref() == Some(LEGACY_STOP),
        "reviewer adoption requires the recorded legacy candidate-echo stop"
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
        .get(&stage.reviewer)
        .context("unknown reviewer role")?;
    let worker: Worktree = serde_json::from_value(record.worktree.clone())?;
    ensure!(
        worker.run_dir == store.root() && worker.path == store.root().join("worktree"),
        "worker worktree ownership mismatch"
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
    ensure!(
        plan.session_in.is_none(),
        "reviewer session provenance must not be resumed"
    );
    let outcome: provider::InvocationOutcome = read_json(&action.join("outcome.json"))?;
    let failure: Value = read_json(&action.join("failure.json"))?;
    ensure!(
        failure["error"] == LEGACY_STOP,
        "failure record does not match recorded stop"
    );
    let review_dir = store.root().join("reviews").join(expected_action);
    directory(&store.root().join("reviews"))?;
    directory(&review_dir)?;
    let review: Worktree = serde_json::from_value(
        packet
            .get("review_worktree")
            .cloned()
            .context("review worktree record missing")?,
    )?;
    ensure!(
        review.source == worker.source
            && review.run_dir == review_dir
            && review.path == review_dir.join("worktree")
            && review.baseline == expected_candidate,
        "review worktree ownership mismatch"
    );
    let mut env = BTreeMap::new();
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
    let mut expected_packet = json!({"scratch_env":env, "schema_version":1, "run_id":record.run_id, "action_id":expected_action, "template_digest":snapshot.sha256, "workflow_purpose":workflow.purpose, "run_context":snapshot.run_context, "stage":record.state.stage, "gate":Gate::Reviewer, "role":stage.reviewer, "baseline_commit":record.state.accepted_commit, "candidate_commit":record.state.candidate_commit, "inputs":stage.inputs, "protected_inputs":stage.protected_inputs, "outputs":stage.outputs, "output_schema":stage.output_schema.as_ref().and_then(|p| snapshot.files.get(p)), "instructions":instructions, "profile":profile, "feedback":record.state.feedback, "unblock_origin":record.state.unblock_origin, "review_routes":stage.routes.keys().collect::<Vec<_>>(), "last_transition":record.state.last_transition, "review_worktree":review});
    if stage.route_selector.as_deref() == Some("worker") {
        expected_packet["review_routes"] = Value::Null;
        expected_packet["candidate_route"] = json!(record.state.candidate_route);
    }
    ensure!(
        packet == expected_packet,
        "recorded action packet differs from frozen Reviewer authority"
    );
    let prompt = crate::controller::action_prompt(&packet)?;
    let mut expected_plan = provider::prepare_with_session(
        role,
        Gate::Reviewer,
        &prompt,
        &review.path,
        &packet_path,
        None,
    )?;
    ensure!(
        plan.program != "/usr/bin/sandbox-exec",
        "legacy sandbox-wrapped Reviewer records require explicit reconciliation"
    );
    expected_plan.scratch = Some(scratch);
    expected_plan.env = env;
    ensure!(
        serde_json::to_value(&plan)? == serde_json::to_value(&expected_plan)?,
        "recorded invocation differs from frozen Reviewer plan"
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
            .context("Reviewer response missing")?,
    )?;
    ensure!(
        response.action_id == expected_action
            && response.outcome == "pass"
            && response.candidate_commit.as_deref() == Some(expected_candidate)
            && response.inspected_commit.as_deref() == Some(expected_candidate),
        "recorded review did not pass the exact expected candidate"
    );
    // Persist the check before invoking Git, whose filters may have effects.
    if record.state.status == Status::Running {
        record.state.status = Status::AdoptionChecking;
        store.save(&record)?;
    }
    worker.verify_candidate(expected_candidate, &record.state.accepted_commit)?;
    review.verify_candidate(expected_candidate, expected_candidate)?;
    review.verify_pristine()?;
    artifact::inputs(&worker.path, expected_candidate, stage)?;
    artifact::frozen_paths(
        &worker.path,
        &worker.baseline,
        expected_candidate,
        &workflow.frozen_project_files,
    )?;
    artifact::frozen_paths(
        &worker.path,
        &record.state.accepted_commit,
        expected_candidate,
        &stage.protected_inputs,
    )?;
    artifact::outputs(&worker.path, expected_candidate, stage, &snapshot)?;
    record.state.status = Status::Running;
    record.state.stop_reason = None;
    record.state.apply(&workflow, response)?;
    ensure!(
        record.state.status != Status::Running,
        "review adoption did not complete the recorded action"
    );
    store.save(&record)?;
    Ok(record)
}
