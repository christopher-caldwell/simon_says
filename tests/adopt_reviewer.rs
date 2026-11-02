#![cfg(target_os = "macos")]
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn git(repo: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().into()
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(args)
        .output()
        .unwrap()
}
fn state(run: &Path) -> Value {
    serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap()
}
fn fixture() -> (tempfile::TempDir, PathBuf, String) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for e in fs::read_dir("examples/data-report").unwrap() {
        let e = e.unwrap();
        if e.path().is_file() {
            fs::copy(e.path(), repo.join(e.file_name())).unwrap();
        }
    }
    let path = repo.join("workflow_action.py");
    let text = fs::read_to_string(&path).unwrap();
    let old = "\"report\": report, \"candidate_commit\": candidate,";
    let new="\"report\": report, \"candidate_commit\": ('wrong' if packet['gate'] == 'reviewer' else candidate),";
    assert!(text.contains(old));
    fs::write(path, text.replace(old, new)).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = tmp.path().join("run");
    let out = cli(&[
        "run",
        repo.join("workflow.toml").to_str().unwrap(),
        "--project",
        repo.to_str().unwrap(),
        "--run-dir",
        run.to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    let mut record = state(&run);
    assert_eq!(record["state"]["gate"], "reviewer");
    let candidate = record["state"]["candidate_commit"]
        .as_str()
        .unwrap()
        .to_owned();
    // Recreate a legacy candidate-echo stop with mutually consistent captured records.
    let action = run.join("actions/action-1");
    let mut outcome: Value =
        serde_json::from_slice(&fs::read(action.join("outcome.json")).unwrap()).unwrap();
    let mut response: Value = serde_json::from_str(outcome["response"].as_str().unwrap()).unwrap();
    response["candidate_commit"] = candidate.clone().into();
    let raw = serde_json::to_string(&response).unwrap() + "\n";
    outcome["response"] = raw.clone().into();
    fs::write(action.join("stdout.txt"), &raw).unwrap();
    fs::write(
        action.join("outcome.json"),
        serde_json::to_vec(&outcome).unwrap(),
    )
    .unwrap();
    let legacy = "reviewer cannot nominate candidate";
    record["state"]["stop_reason"] = legacy.into();
    fs::write(run.join("state.json"), serde_json::to_vec(&record).unwrap()).unwrap();
    fs::write(
        action.join("failure.json"),
        serde_json::to_vec(&json!({"error":legacy})).unwrap(),
    )
    .unwrap();
    (tmp, run, candidate)
}
fn adopt(run: &Path, candidate: &str, ack: bool) -> Output {
    let mut args = vec![
        "adopt-reviewer",
        run.to_str().unwrap(),
        "--expected-action",
        "action-1",
        "--expected-candidate",
        candidate,
    ];
    if ack {
        args.push("--acknowledge-external-effects");
    }
    cli(&args)
}
#[test]
fn accepts_recorded_exact_review_without_replaying_it() {
    let (_tmp, run, candidate) = fixture();
    assert!(!adopt(&run, &candidate, false).status.success());
    let out = adopt(&run, &candidate, true);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state(&run)["state"]["status"], "ready");
    assert_eq!(state(&run)["state"]["stage"], "report");
    assert!(!run.join("actions/action-2").exists());
    assert!(!adopt(&run, &candidate, true).status.success());
}
#[test]
fn refuses_wrong_identity_and_modified_review_checkout() {
    let (_tmp, run, candidate) = fixture();
    assert!(!adopt(&run, "wrong", true).status.success());
    fs::write(run.join("reviews/action-1/worktree/input.json"), "{}\n").unwrap();
    assert!(!adopt(&run, &candidate, true).status.success());
    assert_eq!(state(&run)["state"]["status"], "adoption_checking");
    assert!(!run.join("actions/action-2").exists());
}

#[test]
fn refuses_a_review_echoing_a_different_commit() {
    let (_tmp, run, candidate) = fixture();
    let action = run.join("actions/action-1");
    let mut outcome: Value =
        serde_json::from_slice(&fs::read(action.join("outcome.json")).unwrap()).unwrap();
    let mut response: Value = serde_json::from_str(outcome["response"].as_str().unwrap()).unwrap();
    response["candidate_commit"] = "different".into();
    let raw = serde_json::to_string(&response).unwrap() + "\n";
    outcome["response"] = raw.clone().into();
    fs::write(action.join("stdout.txt"), raw).unwrap();
    fs::write(
        action.join("outcome.json"),
        serde_json::to_vec(&outcome).unwrap(),
    )
    .unwrap();
    assert!(!adopt(&run, &candidate, true).status.success());
    assert_eq!(state(&run)["state"]["status"], "running");
    assert!(!run.join("actions/action-2").exists());
}
