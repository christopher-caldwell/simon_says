#![cfg(target_os = "macos")]

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(args)
        .output()
        .unwrap()
}
fn success(out: &Output) {
    assert!(
        out.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
fn json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn setup() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for e in fs::read_dir("examples/dynamic-repair").unwrap() {
        let e = e.unwrap();
        fs::copy(e.path(), repo.join(e.file_name())).unwrap();
    }
    fs::write(repo.join("mode.json"), "{\"mode\":\"repairable\"}\n").unwrap();
    fs::write(repo.join("engine.json"), "{\"correct\":false}\n").unwrap();
    let file = repo.join("workflow.toml");
    let text = fs::read_to_string(&file).unwrap().replacen(
        "on_pass = \"complete\"",
        "approval_required = true\non_pass = \"complete\"",
        1,
    );
    fs::write(file, text).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = temp.path().join("run");
    let out = cli(&[
        "run",
        repo.join("workflow.toml").to_str().unwrap(),
        "--project",
        repo.to_str().unwrap(),
        "--run-dir",
        run.to_str().unwrap(),
    ]);
    success(&out);
    let record = json(&run.join("state.json"));
    assert_eq!(record["state"]["status"], "awaiting_approval");
    assert_eq!(record["state"]["pending_approval"]["route"], "issues");
    (temp, run)
}
fn approve(run: &Path, state: &Value) -> Output {
    cli(&[
        "approve",
        run.to_str().unwrap(),
        "--expected-commit",
        state["state"]["accepted_commit"].as_str().unwrap(),
        "--expected-digest",
        state["snapshot_digest"].as_str().unwrap(),
        "--expected-approval",
        state["state"]["pending_approval"]["approval_id"]
            .as_str()
            .unwrap(),
    ])
}

#[test]
fn approved_route_dispatches_reviewed_repair_after_a_fresh_process() {
    let (_temp, run) = setup();
    let before = json(&run.join("state.json"));
    success(&approve(&run, &before));
    let packet = json(&run.join("actions/action-2/packet.json"));
    assert_eq!(packet["role"], "repair_specialist");
    assert_eq!(packet["last_transition"]["selected_route"], "issues");
    assert_eq!(packet["last_transition"]["review_action"], "action-1");
    assert_eq!(
        json(&run.join("state.json"))["state"]["status"],
        "awaiting_approval"
    );
}

#[test]
fn changing_pending_route_to_a_different_valid_route_does_not_authorize_it() {
    let (_temp, run) = setup();
    let path = run.join("state.json");
    let mut record = json(&path);
    record["state"]["pending_approval"]["route"] = Value::String("clean".into());
    fs::write(&path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
    let before = fs::read(&path).unwrap();
    let out = approve(&run, &record);
    assert!(
        !out.status.success(),
        "altered route was approved: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        fs::read(&path).unwrap(),
        before,
        "rejected approval changed state"
    );
    assert!(!run.join("actions/action-2").exists());
}

#[test]
fn changed_review_action_response_does_not_match_pending_route() {
    let (_temp, run) = setup();
    let state = json(&run.join("state.json"));
    let path = run.join("actions/action-1/outcome.json");
    let mut outcome = json(&path);
    let mut response: Value = serde_json::from_str(outcome["response"].as_str().unwrap()).unwrap();
    response["route"] = Value::String("clean".into());
    outcome["response"] = Value::String(serde_json::to_string(&response).unwrap());
    fs::write(&path, serde_json::to_vec_pretty(&outcome).unwrap()).unwrap();
    let before = fs::read(run.join("state.json")).unwrap();
    assert!(!approve(&run, &state).status.success());
    assert_eq!(fs::read(run.join("state.json")).unwrap(), before);
    assert!(!run.join("actions/action-2").exists());
}

#[test]
fn symlinked_review_record_is_rejected_without_approval() {
    use std::os::unix::fs::symlink;
    let (_temp, run) = setup();
    let state = json(&run.join("state.json"));
    let path = run.join("actions/action-1/outcome.json");
    let moved = run.join("saved-outcome.json");
    fs::rename(&path, &moved).unwrap();
    symlink(&moved, &path).unwrap();
    let before = fs::read(run.join("state.json")).unwrap();
    assert!(!approve(&run, &state).status.success());
    assert_eq!(fs::read(run.join("state.json")).unwrap(), before);
    assert!(!run.join("actions/action-2").exists());
}

#[test]
fn traversal_review_identity_is_rejected_before_filesystem_access() {
    let (_temp, run) = setup();
    let path = run.join("state.json");
    let mut state = json(&path);
    state["state"]["pending_approval"]["review_action"] = Value::String("../../unexpected".into());
    fs::write(&path, serde_json::to_vec_pretty(&state).unwrap()).unwrap();
    let before = fs::read(&path).unwrap();
    assert!(!approve(&run, &state).status.success());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(!run.join("actions/action-2").exists());
}

#[test]
fn symlinked_actions_parent_is_rejected() {
    use std::os::unix::fs::symlink;
    let (_temp, run) = setup();
    let state = json(&run.join("state.json"));
    fs::rename(run.join("actions"), run.join("saved-actions")).unwrap();
    symlink(run.join("saved-actions"), run.join("actions")).unwrap();
    let before = fs::read(run.join("state.json")).unwrap();
    assert!(!approve(&run, &state).status.success());
    assert_eq!(fs::read(run.join("state.json")).unwrap(), before);
    assert!(!run.join("saved-actions/action-2").exists());
}
