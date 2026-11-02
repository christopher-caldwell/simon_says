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
fn fixture(records: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/data-dispatch").unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
    }
    fs::write(repo.join("records.json"), records).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = temp.path().join("run");
    (temp, repo, run)
}
fn run(repo: &Path, run: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ])
        .output()
        .unwrap()
}
fn json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
#[test]
fn structured_expenses_route_to_normalizer_and_back_without_magic_code() {
    let original =
        "[{\"id\":\"travel\",\"amount\":\" 12.50 \"},{\"id\":\"books\",\"amount\":\"7.25\"}]";
    let (_temp, repo, run_dir) = fixture(original);
    let result = run(&repo, &run_dir);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let state = json(&run_dir.join("state.json"));
    assert_eq!(state["state"]["status"], "complete");
    assert_eq!(state["state"]["stage_transitions"], 3);
    assert_eq!(
        json(&run_dir.join("worktree/audit.json")),
        serde_json::json!({"count":2,"total_cents":1975,"needs_cleanup":0})
    );
    assert_eq!(json(&run_dir.join("worktree/repair.json"))["normalized"], 1);
    assert_eq!(
        fs::read_to_string(repo.join("records.json")).unwrap(),
        original
    );
    let packet = json(&run_dir.join("actions/action-2/packet.json"));
    assert_eq!(packet["role"], "normalizer");
    assert_eq!(packet["last_transition"]["selected_route"], "needs_cleanup");
}
#[test]
fn clean_expenses_skip_normalizer() {
    let (_temp, repo, run_dir) = fixture("[{\"id\":\"books\",\"amount\":\"7.25\"}]");
    let result = run(&repo, &run_dir);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        json(&run_dir.join("state.json"))["state"]["status"],
        "complete"
    );
    assert!(!run_dir.join("actions/action-2").exists());
}
#[test]
fn invalid_amount_stops_without_guessing() {
    let (_temp, repo, run_dir) = fixture("[{\"id\":\"books\",\"amount\":\"unknown\"}]");
    let _result = run(&repo, &run_dir);
    let state = json(&run_dir.join("state.json"));
    assert_eq!(state["state"]["status"], "stopped");
    assert_eq!(state["state"]["stop_reason"], "unblocker blocked");
    assert!(!run_dir.join("actions/action-2").exists());
}
#[test]
fn reviewer_refuses_normalizer_that_changes_amounts() {
    let (_temp, repo, run_dir) = fixture("[{\"id\":\"books\",\"amount\":\" 7.25 \"}]");
    let action = repo.join("action.py");
    let code = fs::read_to_string(&action).unwrap();
    let altered = code.replace(
        "item['amount'] = item['amount'].strip()",
        "item['amount'] = '999.00'",
    );
    assert_ne!(code, altered);
    let altered = altered.replace(
        "report = measure(records)\n        if report != read('audit.json') or not report['needs_cleanup']:",
        "report = read('audit.json')\n        if not report['needs_cleanup']:",
    );
    fs::write(&action, altered).unwrap();
    git(&repo, &["add", "action.py"]);
    git(&repo, &["commit", "-qm", "install faulty test normalizer"]);
    let output = run(&repo, &run_dir);
    let state = json(&run_dir.join("state.json"));
    assert_eq!(
        state["state"]["status"],
        "stopped",
        "state={state} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(state["state"]["stop_reason"], "review cycle limit reached");
    assert!(state["state"]["feedback"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v
            .as_str()
            .unwrap_or("")
            .contains("without changing record values")));
}

#[test]
fn data_auditor_can_select_the_normalizer_and_recheck() {
    let (_temp, repo, run_dir) = fixture("[{\"id\":\"books\",\"amount\":\" 7.25 \"}]");
    let workflow_path = repo.join("workflow.toml");
    let workflow = fs::read_to_string(&workflow_path).unwrap().replacen(
        "[stages.audit.routes]",
        "route_selector = \"worker\"\n[stages.audit.routes]",
        1,
    );
    fs::write(workflow_path, workflow).unwrap();
    let action_path = repo.join("action.py");
    let action = fs::read_to_string(&action_path).unwrap().replace(
        "candidate=commit('audit.json'))",
        "candidate=commit('audit.json'), route='needs_cleanup' if report['needs_cleanup'] else 'done')")
        .replace("respond(packet, 'pass', 'Verified audit and selected declared route.', inspected=head, route=route)",
        "respond(packet, 'pass' if packet['candidate_route'] == route else 'changes_required', 'Verified audit and proposed route.', inspected=head)");
    fs::write(action_path, action).unwrap();
    git(&repo, &["add", "workflow.toml", "action.py"]);
    git(
        &repo,
        &["commit", "-qm", "Configure auditor-directed routes"],
    );
    let result = run(&repo, &run_dir);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        json(&run_dir.join("state.json"))["state"]["status"],
        "complete"
    );
    assert_eq!(
        json(&run_dir.join("actions/action-1/packet.json"))["candidate_route"],
        "needs_cleanup"
    );
    assert_eq!(
        json(&run_dir.join("actions/action-2/packet.json"))["role"],
        "normalizer"
    );
    assert_eq!(
        json(&run_dir.join("worktree/audit.json"))["needs_cleanup"],
        0
    );
}
