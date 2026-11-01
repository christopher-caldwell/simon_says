#![cfg(target_os = "macos")]
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
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
fn fixture(require_approval: bool) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for item in fs::read_dir("examples/role-handoff").unwrap() {
        let item = item.unwrap();
        fs::copy(item.path(), repo.join(item.file_name())).unwrap();
    }
    if !require_approval {
        let p = repo.join("workflow.toml");
        let data = fs::read_to_string(&p)
            .unwrap()
            .replace("approval_required = true\n", "");
        fs::write(p, data).unwrap();
    }
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let context = root.path().join("task.txt");
    fs::write(&context, "How many observations were recorded?\n").unwrap();
    let run = root.path().join("run");
    (root, repo, run, context)
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(args)
        .output()
        .unwrap()
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn status(run: &Path) -> Value {
    let output = cli(&["status", run.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn run(repo: &Path, run: &Path, context: &Path) -> Output {
    cli(&[
        "run",
        repo.join("workflow.toml").to_str().unwrap(),
        "--project",
        repo.to_str().unwrap(),
        "--run-dir",
        run.to_str().unwrap(),
        "--context",
        context.to_str().unwrap(),
    ])
}
#[test]
fn roles_preserve_question_and_require_exact_approval_before_finalization() {
    let (_root, repo, run_dir, context) = fixture(true);
    let result = run(&repo, &run_dir, &context);
    assert!(
        result.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let state = status(&run_dir);
    assert_eq!(state["state"]["status"], "awaiting_approval");
    assert_eq!(state["state"]["stage"], "fix");
    assert!(!run_dir.join("actions/action-8").exists());
    assert!(!run_dir.join("worktree/final.json").exists());
    for (index, stage, role) in [
        (0, "investigate", "investigator"),
        (2, "supervise", "supervisor"),
        (4, "analyze", "analyzer"),
        (6, "fix", "fixer"),
    ] {
        let p = read(&run_dir.join(format!("actions/action-{index}/packet.json")));
        assert_eq!(p["stage"], stage);
        assert_eq!(p["role"], role);
        assert_eq!(p["run_context"], "How many observations were recorded?\n");
    }
    let checkout = run_dir.join("worktree");
    let investigation = read(&checkout.join("investigation.json"));
    let collection = read(&checkout.join("collection.json"));
    let analysis = read(&checkout.join("analysis.json"));
    let fixes = read(&checkout.join("fixes.json"));
    assert_eq!(
        investigation["question"],
        "How many observations were recorded?"
    );
    assert!(investigation.get("answer").is_none());
    assert!(investigation.get("observation_count").is_none());
    assert_eq!(collection["observations"].as_array().unwrap().len(), 3);
    assert!(collection.get("recorded_count").is_none());
    assert_eq!(collection["analysis_performed"], false);
    assert!(collection.get("answer").is_none());
    assert_eq!(collection["backlog"][0]["status"], "open");
    assert_eq!(analysis["question"], investigation["question"]);
    assert_eq!(analysis["answer"], "3 observations were recorded.");
    assert_eq!(analysis["unresolved_issues"][0], "MISSING_CONTEXT");
    assert_eq!(fixes["processed_ids"][0], "MISSING_CONTEXT");
    assert_eq!(fixes["disposition"], "recorded_for_follow_up");
    let id = state["state"]["pending_approval"]["approval_id"]
        .as_str()
        .unwrap();
    let commit = state["state"]["accepted_commit"].as_str().unwrap();
    let digest = state["snapshot_digest"].as_str().unwrap();
    for (challenge, expected_commit, expected_digest) in [
        ("wrong", commit, digest),
        (id, "wrong", digest),
        (id, commit, "wrong"),
    ] {
        let wrong = cli(&[
            "approve",
            run_dir.to_str().unwrap(),
            "--expected-approval",
            challenge,
            "--expected-commit",
            expected_commit,
            "--expected-digest",
            expected_digest,
        ]);
        assert!(!wrong.status.success());
        assert_eq!(status(&run_dir)["state"]["status"], "awaiting_approval");
        assert!(!run_dir.join("actions/action-8").exists());
    }
    let approved = cli(&[
        "approve",
        run_dir.to_str().unwrap(),
        "--expected-approval",
        id,
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--note",
        "fixture test approval",
    ]);
    assert!(
        approved.status.success(),
        "{}",
        String::from_utf8_lossy(&approved.stderr)
    );
    let done = status(&run_dir);
    assert_eq!(done["state"]["status"], "complete");
    assert_eq!(done["state"]["approvals"][0]["approval_id"], id);
    assert_eq!(done["state"]["approvals"][0]["commit"], commit);
    assert_eq!(done["state"]["approvals"][0]["snapshot_digest"], digest);
    let final_output = read(&checkout.join("final.json"));
    assert_eq!(final_output["question"], investigation["question"]);
    assert_eq!(final_output["answer"], analysis["answer"]);
    assert_eq!(
        final_output["backlog_disposition"],
        "recorded_for_follow_up"
    );
    assert!(!repo.join("final.json").exists());
}
#[test]
fn optional_approval_can_be_disabled_by_author_before_run() {
    let (_root, repo, run_dir, context) = fixture(false);
    let output = run(&repo, &run_dir, &context);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let done = status(&run_dir);
    assert_eq!(done["state"]["status"], "complete");
    assert!(done["state"]["approvals"].as_array().unwrap().is_empty());
}
