use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};

fn git(dir: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}
fn action(
    dir: &Path,
    packet_path: &Path,
    stage: &str,
    gate: &str,
    candidate: Option<&str>,
) -> Value {
    fs::write(packet_path, serde_json::to_vec(&json!({"action_id": "test-action", "gate": gate, "stage": stage, "candidate_commit": candidate})).unwrap()).unwrap();
    let result = Command::new("python3")
        .arg("workflow_action.py")
        .env("SIMON_PACKET", packet_path)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "action failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
#[test]
fn fixture_runs_both_stages_and_reviewer_checks_exact_head() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for item in fs::read_dir("examples/data-report").unwrap() {
        let item = item.unwrap();
        fs::copy(item.path(), repo.join(item.file_name())).unwrap();
    }
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "baseline",
        ],
    );
    let packet = temp.path().join("packet.json");
    for stage in ["validate", "report"] {
        let work = action(&repo, &packet, stage, "worker", None);
        assert_eq!(work["outcome"], "complete");
        let head = git(&repo, &["rev-parse", "HEAD"]);
        assert_eq!(work["candidate_commit"], head);
        let review = action(&repo, &packet, stage, "reviewer", Some(&head));
        assert_eq!(review["outcome"], "pass");
        assert_eq!(review["inspected_commit"], head);
        assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    }
    let summary: Value =
        serde_json::from_slice(&fs::read(repo.join("summary.json")).unwrap()).unwrap();
    assert_eq!(summary["mean"], 7.0);
}
#[test]
fn invalid_input_blocks_and_unblocker_stops() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    fs::copy(
        "examples/data-report/workflow_action.py",
        repo.join("workflow_action.py"),
    )
    .unwrap();
    fs::write(
        repo.join("input.json"),
        r#"{"measurements":[{"name":"a","value":true}]}"#,
    )
    .unwrap();
    let packet = temp.path().join("packet.json");
    assert_eq!(
        action(&repo, &packet, "validate", "worker", None)["outcome"],
        "blocked"
    );
    assert_eq!(
        action(&repo, &packet, "validate", "unblocker", None)["outcome"],
        "blocked"
    );
}
