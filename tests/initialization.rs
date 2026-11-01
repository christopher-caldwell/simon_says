use simon_says::{
    controller,
    git::{InitializationIntent, InitializationPhase},
    routing::{State, Status},
    store::{RunRecord, RunStore},
    Snapshot, Workflow,
};
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn setup(
    phase: InitializationPhase,
) -> (tempfile::TempDir, PathBuf, PathBuf, InitializationIntent) {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/data-report").unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
        }
    }
    fs::write(repo.join(".gitignore"), "ignored.tmp\n").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = root.path().join("run");
    let snapshot = Snapshot::create(&repo.join("workflow.toml")).unwrap();
    let workflow: Workflow = toml::from_str(&snapshot.template).unwrap();
    let mut intent = InitializationIntent::capture(&repo, &run).unwrap();
    intent.phase = phase;
    let mut record = RunRecord {
        version: 1,
        run_id: "run".into(),
        snapshot_digest: snapshot.sha256.clone(),
        state: State::new(&workflow, String::new()),
        next_action: 0,
        worktree: serde_json::to_value(&intent).unwrap(),
    };
    record.state.status = Status::Initializing;
    drop(RunStore::create(&run, &snapshot, &record).unwrap());
    (root, repo, run, intent)
}
#[test]
fn reserved_intent_resumes_without_replaying_uncertain_checkout() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, _repo, run, intent) = setup(InitializationPhase::Reserved);
    controller::recover_initialization(&run, false, false, false).unwrap();
    let state = controller::status(&run).unwrap();
    assert_eq!(state.state.status, Status::Ready);
    assert_eq!(state.state.accepted_commit, intent.baseline);
    assert!(run.join("worktree").exists());
    assert!(controller::recover_initialization(&run, true, false, true).is_err());
}
#[test]
fn creating_requires_ack_and_adopts_exact_completed_worktree() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, repo, run, intent) = setup(InitializationPhase::Creating);
    intent.create(&run).unwrap();
    fs::create_dir(run.join("reviews")).unwrap();
    assert!(controller::recover_initialization(&run, false, false, false).is_err());
    assert!(controller::recover_initialization(&run, true, false, false).is_err());
    controller::recover_initialization(&run, true, false, true).unwrap();
    let state = controller::status(&run).unwrap();
    assert_eq!(state.state.status, Status::Ready);
    assert_eq!(state.state.accepted_commit, intent.baseline);
    assert!(!repo.join("validation.json").exists());
}
#[test]
fn missing_checkout_requires_both_ack_and_explicit_retry() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, _repo, run, _intent) = setup(InitializationPhase::Creating);
    assert!(controller::recover_initialization(&run, false, true, false).is_err());
    assert!(controller::recover_initialization(&run, true, false, true).is_err());
    controller::recover_initialization(&run, true, true, false).unwrap();
    assert_eq!(
        controller::status(&run).unwrap().state.status,
        Status::Ready
    );
}
#[test]
fn unsafe_partial_or_dirty_worktrees_are_never_deleted() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, _repo, run, intent) = setup(InitializationPhase::Creating);
    intent.create(&run).unwrap();
    fs::write(run.join("worktree/ignored.tmp"), "keep").unwrap();
    assert!(controller::recover_initialization(&run, true, false, true).is_err());
    assert_eq!(
        fs::read_to_string(run.join("worktree/ignored.tmp")).unwrap(),
        "keep"
    );
    fs::remove_file(run.join("worktree/ignored.tmp")).unwrap();
    fs::write(run.join("worktree/input.json"), "changed").unwrap();
    assert!(controller::recover_initialization(&run, true, false, true).is_err());
    assert!(run.join("worktree/input.json").exists());
}
#[test]
fn missing_registered_checkout_and_legacy_null_intent_fail_closed() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, _repo, run, intent) = setup(InitializationPhase::Creating);
    intent.create(&run).unwrap();
    fs::rename(run.join("worktree"), run.join("moved-worktree")).unwrap();
    assert!(controller::recover_initialization(&run, true, true, false).is_err());
    assert!(run.join("moved-worktree").exists());
    let store = RunStore::open(&run).unwrap();
    let (mut record, _) = store.load().unwrap();
    record.worktree = serde_json::Value::Null;
    store.save(&record).unwrap();
    drop(store);
    assert!(controller::recover_initialization(&run, true, true, false).is_err());
    assert!(run.join("moved-worktree").exists());
}
#[cfg(unix)]
#[test]
fn symlinked_checkout_and_unexpected_action_evidence_are_refused() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, _repo, run, _intent) = setup(InitializationPhase::Creating);
    let external = run.parent().unwrap().join("external");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("keep"), "safe").unwrap();
    std::os::unix::fs::symlink(&external, run.join("worktree")).unwrap();
    assert!(controller::recover_initialization(&run, true, false, true).is_err());
    assert!(external.join("keep").exists());
    fs::remove_file(run.join("worktree")).unwrap();
    fs::write(run.join("actions/unexpected"), "evidence").unwrap();
    assert!(controller::recover_initialization(&run, true, true, false).is_err());
    assert!(run.join("actions/unexpected").exists());
}
#[test]
fn resumed_cli_requires_explicit_ack_for_uncertain_initialization() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, _repo, run, intent) = setup(InitializationPhase::Creating);
    intent.create(&run).unwrap();
    let bin = env!("CARGO_BIN_EXE_simon-says");
    let output = Command::new(bin)
        .args(["recover-init", run.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let output = Command::new(bin)
        .args([
            "recover-init",
            run.to_str().unwrap(),
            "--acknowledge-external-effects",
            "--adopt-existing",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn initialization_rejects_run_inside_external_git_directory_before_creation() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    let common = root.path().join("external.git");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/data-report").unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
        }
    }
    git(
        &repo,
        &["init", "--separate-git-dir", common.to_str().unwrap(), "-q"],
    );
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = common.join("candidate-run");
    let error = controller::initialize(&repo.join("workflow.toml"), &repo, &run).unwrap_err();
    assert!(format!("{error:#}").contains("Git's common directory"));
    assert!(
        !run.exists(),
        "preflight must not create the run inside Git metadata"
    );
    assert!(common.join("objects").is_dir());
    assert!(repo.join("workflow.toml").is_file());
}
