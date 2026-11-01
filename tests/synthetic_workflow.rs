#![cfg(target_os = "macos")]

use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn git(repo: &Path, args: &[&str]) {
    let result = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "git: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
fn setup(exercise: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/synthetic-opening-hand").unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
    }
    let input = repo.join("experiment.json");
    let mut config: Value = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    config["exercise"] = json!(exercise);
    fs::write(input, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = temp.path().join("run");
    (temp, repo, run)
}
fn invoke(repo: &Path, run: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .arg("run")
        .arg(repo.join("workflow.toml"))
        .arg("--project")
        .arg(repo)
        .arg("--run-dir")
        .arg(run)
        .output()
        .unwrap()
}
fn packet(run: &Path, index: usize) -> Value {
    serde_json::from_slice(
        &fs::read(run.join(format!("actions/action-{index}/packet.json"))).unwrap(),
    )
    .unwrap()
}
fn assert_success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn representative_simulation_runs_two_stages_via_cli() {
    let (_temp, repo, run) = setup("normal");
    let result = assert_success(&invoke(&repo, &run));
    assert_eq!(result["state"]["status"], "complete");
    assert_eq!(packet(&run, 0)["stage"], "simulate");
    assert_eq!(packet(&run, 1)["gate"], "reviewer");
    assert_eq!(packet(&run, 2)["stage"], "analyze");
    assert_eq!(packet(&run, 3)["gate"], "reviewer");
    assert!(!repo.join("samples.json").exists());
    assert!(!repo.join("analysis.json").exists());
}
#[test]
fn reviewer_feedback_reaches_worker_then_run_completes() {
    let (_temp, repo, run) = setup("review_once");
    let result = assert_success(&invoke(&repo, &run));
    assert_eq!(result["state"]["status"], "complete");
    let correction = packet(&run, 2);
    assert_eq!(correction["gate"], "worker");
    assert_eq!(correction["stage"], "simulate");
    assert!(correction["feedback"].to_string().contains("revision 2"));
    assert!(correction["candidate_commit"].is_string());
    assert_eq!(packet(&run, 3)["gate"], "reviewer");
}
#[test]
fn one_block_retries_but_repeated_block_stops() {
    let (_temp, repo, run) = setup("block_once");
    let result = assert_success(&invoke(&repo, &run));
    assert_eq!(result["state"]["status"], "complete");
    assert_eq!(packet(&run, 1)["gate"], "unblocker");
    assert_eq!(packet(&run, 1)["unblock_origin"], "worker");
    assert_eq!(packet(&run, 2)["gate"], "worker");
    assert!(packet(&run, 2)["feedback"]
        .to_string()
        .contains("permits one retry"));

    let (_temp, repo, run) = setup("always_block");
    let output = invoke(&repo, &run);
    assert!(!output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["state"]["status"], "stopped");
    assert_eq!(result["state"]["stop_reason"], "repeated block after retry");
    assert_eq!(packet(&run, 1)["gate"], "unblocker");
    assert_eq!(packet(&run, 2)["gate"], "worker");
    assert!(!run.join("actions/action-3").exists());
}
#[test]
fn reviewer_block_retries_exact_candidate() {
    let (_temp, repo, run) = setup("reviewer_block_once");
    let result = assert_success(&invoke(&repo, &run));
    assert_eq!(result["state"]["status"], "complete");
    let before = packet(&run, 1);
    let unblock = packet(&run, 2);
    let retry = packet(&run, 3);
    assert_eq!(before["gate"], "reviewer");
    assert_eq!(unblock["gate"], "unblocker");
    assert_eq!(unblock["unblock_origin"], "reviewer");
    assert_eq!(retry["gate"], "reviewer");
    assert_eq!(before["candidate_commit"], retry["candidate_commit"]);
    assert!(retry["feedback"].to_string().contains("recoverable block"));
}
