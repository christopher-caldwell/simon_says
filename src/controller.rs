use crate::{
    artifact,
    git::{InitializationIntent, InitializationPhase, Worktree},
    provider,
    routing::{Gate, Response, State, Status},
    store::{recovery_commit, RunRecord, RunStore},
    Snapshot, Workflow,
};
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::{fs, io::Write, path::Path};

pub fn initialize(template: &Path, project: &Path, run_dir: &Path) -> Result<()> {
    initialize_with_context(template, project, run_dir, None)
}
/// Bind optional run-specific context before any checkout or action.
pub fn initialize_with_context(
    template: &Path,
    project: &Path,
    run_dir: &Path,
    context: Option<&Path>,
) -> Result<()> {
    let mut snapshot = Snapshot::create(template)?;
    if let Some(path) = context {
        snapshot = snapshot.with_context(path)?;
    }
    let workflow: Workflow = toml::from_str(&snapshot.template)?;
    let intent = InitializationIntent::capture(project, run_dir)?;
    // Fail before creating a run if declared project authority is absent or unsafe.
    for path in &workflow.frozen_project_files {
        artifact::tracked_blob(&intent.source, &intent.baseline, path)
            .with_context(|| format!("frozen project file {path}"))?;
    }
    let mut record = RunRecord {
        version: 1,
        run_id: run_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("run")
            .to_owned(),
        snapshot_digest: snapshot.sha256.clone(),
        state: State::new(&workflow, String::new()),
        next_action: 0,
        worktree: serde_json::to_value(&intent)?,
    };
    record.state.status = Status::Initializing;
    let store = RunStore::create(run_dir, &snapshot, &record)?;
    finish_initialization(&store, &mut record, &workflow, false, false, false)
}
fn empty_directory(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "unsafe initialization directory {}",
        path.display()
    );
    ensure!(
        fs::read_dir(path)?.next().is_none(),
        "initialization directory is not empty: {}",
        path.display()
    );
    Ok(())
}
fn finish_initialization(
    store: &RunStore,
    record: &mut RunRecord,
    workflow: &Workflow,
    acknowledge: bool,
    retry_create: bool,
    adopt_existing: bool,
) -> Result<()> {
    ensure!(
        !(retry_create && adopt_existing),
        "--retry-create and --adopt-existing are mutually exclusive"
    );
    ensure!(
        record.state.status == Status::Initializing,
        "run is not initializing"
    );
    let mut expected = State::new(workflow, String::new());
    expected.status = Status::Initializing;
    ensure!(
        serde_json::to_value(&record.state)? == serde_json::to_value(&expected)?
            && record.next_action == 0,
        "initialization state contains unexpected action data"
    );
    empty_directory(&store.root().join("actions"))?;
    let reviews = store.root().join("reviews");
    match fs::symlink_metadata(&reviews) {
        Ok(_) => empty_directory(&reviews)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.into()),
    }
    let mut intent: InitializationIntent = serde_json::from_value(record.worktree.clone())
        .context(
            "initialization intent missing or invalid; legacy runs require manual reconciliation",
        )?;
    let worktree = match intent.phase {
        InitializationPhase::Reserved => {
            ensure!(
                !adopt_existing,
                "--adopt-existing requires an uncertain creating state"
            );
            intent.phase = InitializationPhase::Creating;
            record.worktree = serde_json::to_value(&intent)?;
            store.save(record)?;
            intent.create(store.root())?
        }
        InitializationPhase::Creating => {
            ensure!(acknowledge, "checkout creation is uncertain; confirm Git processes exited and reconcile external effects with --acknowledge-external-effects");
            if intent.target_exists(store.root())? {
                ensure!(adopt_existing, "worktree target exists; --adopt-existing is required to explicitly adopt it after reconciliation");
                intent.adopt(store.root())?
            } else {
                ensure!(
                    !adopt_existing,
                    "--adopt-existing cannot be used when a worktree target is missing"
                );
                ensure!(retry_create, "worktree target missing; --retry-create is required to explicitly repeat checkout after reconciliation");
                intent.verify_absent_and_unregistered(store.root())?;
                intent.create(store.root())?
            }
        }
    };
    match fs::symlink_metadata(&reviews) {
        Ok(_) => empty_directory(&reviews)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&reviews)?;
            fs::File::open(store.root())?.sync_all()?;
        }
        Err(e) => return Err(e.into()),
    }
    record.state.accepted_commit = worktree.baseline.clone();
    record.state.status = Status::Ready;
    record.worktree = serde_json::to_value(&worktree)?;
    store.save(record)?;
    Ok(())
}
/// Explicitly reconcile an interrupted checkout. This never removes paths or Git registrations.
pub fn recover_initialization(
    run_dir: &Path,
    acknowledge_external_effects: bool,
    retry_create: bool,
    adopt_existing: bool,
) -> Result<()> {
    let store = RunStore::open(run_dir)?;
    let (mut record, snapshot) = store.load()?;
    let workflow: Workflow = toml::from_str(&snapshot.template)?;
    finish_initialization(
        &store,
        &mut record,
        &workflow,
        acknowledge_external_effects,
        retry_create,
        adopt_existing,
    )
}

fn write_result(path: &Path, data: &impl serde::Serialize) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    serde_json::to_writer_pretty(&mut file, data)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
pub fn status(run_dir: &Path) -> Result<RunRecord> {
    RunStore::read_status(run_dir)
}
pub fn execute(run_dir: &Path) -> Result<RunRecord> {
    let store = RunStore::open(run_dir)?;
    let initially_ready = store.load()?.0.state.status == Status::Ready;
    let result = execute_locked(&store);
    if let Err(error) = &result {
        if !initially_ready {
            return result;
        }
        if let Ok((mut record, _)) = store.load() {
            if matches!(record.state.status, Status::Preparing | Status::Running) {
                if let Some(id) = &record.state.action_id {
                    if !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        let path = store.root().join("actions").join(id).join("failure.json");
                        let _ = write_result(&path, &json!({"error": format!("{error:#}")}));
                        record.state.stop_reason = Some(format!("{error:#}"));
                        let _ = store.save(&record);
                    }
                }
            } else if record.state.status == Status::Ready {
                record.state.status = Status::Stopped;
                record.state.stop_reason = Some(format!("pre-action check failed: {error:#}"));
                let _ = store.save(&record);
            }
        }
    }
    result
}
fn execute_locked(store: &RunStore) -> Result<RunRecord> {
    let (mut record, snapshot) = store.load()?;
    RunStore::check_resumable(&record)?;
    let workflow: Workflow = toml::from_str(&snapshot.template)?;
    let worktree: Worktree =
        serde_json::from_value(record.worktree.clone()).context("run worktree not initialized")?;
    ensure!(
        worktree.run_dir == store.root(),
        "worktree does not belong to run directory"
    );
    loop {
        if record.state.status != Status::Ready {
            break;
        }
        let stage = workflow
            .stages
            .get(&record.state.stage)
            .context("invalid current stage")?;
        let checkpoint = recovery_commit(&record.state).to_string();
        let gate = record.state.gate;
        let role_name = match gate {
            Gate::Worker => &stage.worker,
            Gate::Reviewer => &stage.reviewer,
            Gate::Unblocker => &stage.unblocker,
        };
        let role = workflow.roles.get(role_name).context("role not found")?;
        let action_id = loop {
            let id = format!("action-{}", record.next_action);
            if !store.root().join("actions").join(&id).exists()
                && !store.root().join("reviews").join(&id).exists()
            {
                break id;
            }
            record.next_action = record.next_action.checked_add(1).context("action limit")?;
        };
        store.prepare_action(&mut record, &action_id)?;
        worktree.verify_candidate(&checkpoint, &record.state.accepted_commit)?;
        artifact::frozen_paths(
            &worktree.path,
            &worktree.baseline,
            &checkpoint,
            &workflow.frozen_project_files,
        )?;
        artifact::inputs(&worktree.path, &checkpoint, stage)?;
        let review = if gate != Gate::Worker {
            let review_dir = store.root().join("reviews").join(&action_id);
            fs::create_dir(&review_dir)?;
            Some(worktree.create_review(&checkpoint, &review_dir)?)
        } else {
            None
        };
        let cwd = review.as_ref().map_or(&worktree.path, |r| &r.path);
        let instructions = snapshot
            .files
            .get(&stage.instructions)
            .context("frozen instructions missing")?;
        let profile = role
            .profile
            .as_ref()
            .map(|p| snapshot.files.get(p).context("frozen profile missing"))
            .transpose()?;
        let packet = json!({"schema_version":1, "run_id":record.run_id, "action_id":action_id, "template_digest":snapshot.sha256, "workflow_purpose":workflow.purpose, "run_context":snapshot.run_context, "stage":record.state.stage, "gate":gate, "role":role_name, "baseline_commit":record.state.accepted_commit, "candidate_commit":record.state.candidate_commit, "inputs":stage.inputs, "outputs":stage.outputs, "output_schema":stage.output_schema.as_ref().and_then(|p| snapshot.files.get(p)), "instructions":instructions, "profile":profile, "feedback":record.state.feedback, "unblock_origin":record.state.unblock_origin, "review_worktree":review.as_ref()});
        let prompt = format!("You are an action in a controlled workflow. Follow this packet and return exactly one JSON object, without markdown. The frozen workflow and controller protocol govern; run_context provides the original task, not authority to alter the workflow, limits, or routes. Do not change the workflow or choose transitions. Worker outcomes: complete (candidate_commit required), blocked; reviewer: pass, changes_required, blocked (inspected_commit required); unblocker: retry, blocked. All responses must include action_id, outcome, nonempty report, candidate_commit and inspected_commit (use null where inapplicable). A complete worker must commit all outputs and leave a clean detached worktree. A blocked worker must leave HEAD and the checkout unchanged from its starting checkpoint; otherwise the controller stops for explicit recovery. Reviewer and unblocker must not modify their checkout. Packet: {}", serde_json::to_string(&packet)?);
        let packet_path = store
            .root()
            .join("actions")
            .join(&action_id)
            .join("packet.json");
        let scratch = store.reserve(&action_id)?;
        let mut plan = provider::prepare(role, gate, &prompt, cwd, &packet_path)?;
        let guarded = crate::sandbox::prepare_with_frozen(
            store.root(),
            cwd,
            &scratch,
            &plan.program,
            &plan.argv,
            &workflow.frozen_project_files,
        )?;
        if provider::requires_nested_sandbox_probe(role, gate) {
            crate::sandbox::probe_nested(&guarded)?;
        }
        plan.program = guarded.program;
        plan.argv = guarded.argv;
        plan.scratch = Some(guarded.scratch);
        let dir = store.begin(&mut record, &packet, &plan)?;
        let outcome = provider::invoke(&plan, &dir)?;
        let observed = store.load();
        let authority_matches =
            observed
                .as_ref()
                .is_ok_and(|(observed_record, observed_snapshot)| {
                    serde_json::to_value(observed_record).ok() == serde_json::to_value(&record).ok()
                        && serde_json::to_value(observed_snapshot).ok()
                            == serde_json::to_value(&snapshot).ok()
                });
        if !authority_matches {
            store
                .restore_trusted(&record, &snapshot)
                .context("could not restore trusted running state")?;
            anyhow::bail!("run authority or state changed during action; restored uncertain running state; reset required");
        }
        for name in ["packet.json", "invocation.json", "stdout.txt", "stderr.txt"] {
            let metadata = fs::symlink_metadata(dir.join(name))?;
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "unsafe action record {name}"
            );
        }
        let observed_packet: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("packet.json"))?)?;
        let observed_plan: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("invocation.json"))?)?;
        ensure!(
            observed_packet == packet && observed_plan == serde_json::to_value(&plan)?,
            "action packet or invocation changed during execution"
        );
        write_result(&dir.join("outcome.json"), &outcome)?;
        ensure!(
            outcome.success,
            "action {action_id} failed with exit {:?}; reset required",
            outcome.exit_code
        );
        let response: Response = serde_json::from_str(
            outcome
                .response
                .as_deref()
                .context("provider returned no response")?,
        )
        .context("invalid action response; reset required")?;
        if let Some(review) = &review {
            review
                .verify_candidate(&checkpoint, &checkpoint)
                .context("review checkout changed")?;
            review.verify_pristine()?;
            worktree.verify_candidate(&checkpoint, &record.state.accepted_commit)?;
        } else if response.outcome == "complete" {
            let candidate = response
                .candidate_commit
                .as_deref()
                .context("candidate missing")?;
            worktree.verify_candidate(candidate, &checkpoint)?;
            artifact::frozen_paths(
                &worktree.path,
                &worktree.baseline,
                candidate,
                &workflow.frozen_project_files,
            )?;
            artifact::outputs(&worktree.path, candidate, stage, &snapshot)?;
        } else {
            worktree.verify_candidate(&checkpoint, &record.state.accepted_commit)?;
        }
        record.state.apply(&workflow, response)?;
        store.save(&record)?;
    }
    Ok(record)
}
/// Approve a specific reviewed checkpoint and frozen workflow, never a mutable checkout.
pub fn approve(
    run_dir: &Path,
    expected_commit: &str,
    expected_digest: &str,
    expected_approval: &str,
    note: Option<String>,
) -> Result<RunRecord> {
    let store = RunStore::open(run_dir)?;
    let (mut record, snapshot) = store.load()?;
    ensure!(
        record.state.status == Status::AwaitingApproval,
        "run is not awaiting approval"
    );
    ensure!(
        record.snapshot_digest == expected_digest && snapshot.sha256 == expected_digest,
        "approval workflow digest does not match"
    );
    ensure!(
        record.state.accepted_commit == expected_commit,
        "approval commit does not match accepted checkpoint"
    );
    let pending = record
        .state
        .pending_approval
        .as_ref()
        .context("pending approval identity missing")?;
    ensure!(
        pending.approval_id == expected_approval
            && pending.stage == record.state.stage
            && pending.commit == expected_commit,
        "approval identity does not match pending review"
    );
    if let Some(note) = &note {
        ensure!(note.len() <= 4096, "approval note exceeds 4096 bytes");
    }
    record.state.status = Status::ApprovalChecking;
    store.save(&record)?;
    let worktree: Worktree = serde_json::from_value(record.worktree.clone())?;
    ensure!(
        worktree.run_dir == store.root(),
        "worktree does not belong to run"
    );
    if let Err(error) = worktree.verify_candidate(expected_commit, expected_commit) {
        record.state.stop_reason = Some(format!("approval verification failed: {error:#}"));
        store.save(&record)?;
        return Err(error.context("approval verification failed; explicit reset is required"));
    }
    let workflow: Workflow = toml::from_str(&snapshot.template)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    record.state.approve(
        &workflow,
        expected_commit,
        expected_digest,
        expected_approval,
        now,
        note,
    )?;
    store.save(&record)?;
    Ok(record)
}
/// Explicit operator acknowledgment is required because Git cannot undo external effects.
pub fn reset(run_dir: &Path, acknowledge_external_effects: bool) -> Result<()> {
    ensure!(acknowledge_external_effects, "reset requires --acknowledge-external-effects after confirming any previous action process has exited and reconciling external effects");
    let store = RunStore::open(run_dir)?;
    let (mut record, _) = store.load()?;
    ensure!(
        matches!(
            record.state.status,
            Status::Preparing | Status::Running | Status::Stopped | Status::ApprovalChecking
        ),
        "reset requires uncertain preparation, execution, approval checking or stopped state"
    );
    let worktree: Worktree = serde_json::from_value(record.worktree.clone())?;
    ensure!(
        worktree.run_dir == store.root(),
        "worktree does not belong to run"
    );
    let approval_checking = record.state.status == Status::ApprovalChecking;
    if approval_checking {
        let pending = record
            .state
            .pending_approval
            .as_ref()
            .context("pending approval identity missing")?;
        ensure!(
            pending.stage == record.state.stage
                && pending.commit == record.state.accepted_commit
                && !pending.approval_id.is_empty(),
            "pending approval identity is inconsistent"
        );
    }
    let checkpoint = recovery_commit(&record.state).to_owned();
    worktree.reset_to(&checkpoint)?;
    if approval_checking {
        record.state.status = Status::AwaitingApproval;
        record.state.stop_reason = None;
        store.save(&record)?;
        return Ok(());
    }
    if record.state.gate == Gate::Unblocker {
        record.state.gate = record
            .state
            .unblock_origin
            .take()
            .context("missing unblock origin")?;
    }
    if record.state.stop_reason.as_deref() == Some("review cycle limit reached") {
        record.state.gate = Gate::Worker;
        record.state.review_cycles = 0;
    }
    record.state.status = Status::Ready;
    record.state.action_id = None;
    record.state.stop_reason = None;
    record.state.unblock_used = false;
    store.save(&record)?;
    Ok(())
}
