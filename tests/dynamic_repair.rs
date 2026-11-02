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

fn fixture_for(example: &str, mode: &str, correct: bool) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir(format!("examples/{example}")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
    }
    fs::write(repo.join("mode.json"), format!("{{\"mode\":\"{mode}\"}}\n")).unwrap();
    fs::write(
        repo.join("engine.json"),
        format!("{{\"correct\":{correct}}}\n"),
    )
    .unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = temp.path().join("run");
    (temp, repo, run)
}

fn fixture(mode: &str, correct: bool) -> (tempfile::TempDir, PathBuf, PathBuf) {
    fixture_for("dynamic-repair", mode, correct)
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

fn action_response(run: &Path, index: usize) -> Value {
    let outcome = read(&run.join(format!("actions/action-{index}/outcome.json")));
    serde_json::from_str(outcome["response"].as_str().unwrap()).unwrap()
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

fn run(repo: &Path, run: &Path) -> Output {
    cli(&[
        "run",
        repo.join("workflow.toml").to_str().unwrap(),
        "--project",
        repo.to_str().unwrap(),
        "--run-dir",
        run.to_str().unwrap(),
    ])
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn direct_fixture(mode: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let (temp, repo, run) = fixture_for("worker-dispatch", mode, false);
    let path = repo.join("workflow.toml");
    let definition = fs::read_to_string(&path)
        .unwrap()
        .replace("reviewer = \"quality_reviewer\"\n", "")
        .replace("unblocker = \"stop\"\n", "")
        .replace("max_review_cycles = 2\n", "")
        .replace("route_selector = \"worker\"\n", "");
    fs::write(path, definition).unwrap();
    git(&repo, &["add", "workflow.toml"]);
    git(&repo, &["commit", "-qm", "direct routing"]);
    (temp, repo, run)
}

#[test]
fn direct_roles_route_issue_to_fixer_and_recheck_without_review_actions() {
    let (_temp, repo, run_dir) = direct_fixture("repairable");
    success(run(&repo, &run_dir));
    assert_eq!(status(&run_dir)["state"]["status"], "complete");
    for (index, role) in ["simulator", "repair_specialist", "simulator"]
        .iter()
        .enumerate()
    {
        let packet = read(&run_dir.join(format!("actions/action-{index}/packet.json")));
        assert_eq!(packet["role"], *role);
    }
    assert_eq!(read(&run_dir.join("worktree/engine.json"))["correct"], true);
    assert!(!run_dir.join("actions/action-3").exists());
}

#[test]
fn direct_role_cannot_select_an_undeclared_route() {
    let (_temp, repo, run_dir) = direct_fixture("unauthorized_route");
    assert!(!run(&repo, &run_dir).status.success());
    assert!(!run_dir.join("actions/action-1").exists());
    assert_eq!(status(&run_dir)["state"]["stage"], "simulate");
}

#[test]
fn direct_handoff_rejects_invalid_output_before_dispatching_next_role() {
    let (_temp, repo, run_dir) = direct_fixture("repairable");
    fs::write(
        repo.join("result.schema.json"),
        r#"{"type":"object","required":["missing"]}"#,
    )
    .unwrap();
    git(&repo, &["add", "result.schema.json"]);
    git(&repo, &["commit", "-qm", "require missing output"]);
    assert!(!run(&repo, &run_dir).status.success());
    assert_eq!(status(&run_dir)["state"]["stage"], "simulate");
    assert!(!run_dir.join("actions/action-1").exists());
}

#[test]
fn issue_dispatches_authored_repair_role_and_rechecks_the_fixed_candidate() {
    let (_temp, repo, run_dir) = fixture("repairable", false);
    success(run(&repo, &run_dir));
    assert_eq!(status(&run_dir)["state"]["status"], "complete");
    let expected = [
        ("simulate", "simulator"),
        ("simulate", "quality_reviewer"),
        ("repair", "repair_specialist"),
        ("repair", "quality_reviewer"),
        ("simulate", "simulator"),
        ("simulate", "quality_reviewer"),
    ];
    for (index, (stage, role)) in expected.iter().enumerate() {
        let packet = read(&run_dir.join(format!("actions/action-{index}/packet.json")));
        assert_eq!(packet["stage"], *stage);
        assert_eq!(packet["role"], *role);
        if index % 2 == 1 {
            let candidate = packet["candidate_commit"].as_str().unwrap();
            let outcome = action_response(&run_dir, index);
            assert_eq!(outcome["inspected_commit"], candidate);
        }
    }
    assert!(!run_dir.join("actions/action-6").exists());
    let repair_packet = read(&run_dir.join("actions/action-2/packet.json"));
    let selected = &repair_packet["last_transition"];
    assert_eq!(selected["from_stage"], "simulate");
    assert_eq!(selected["to_stage"], "repair");
    assert_eq!(selected["selected_route"], "issues");
    assert_eq!(selected["review_action"], "action-1");
    assert_eq!(
        selected["reviewed_commit"],
        action_response(&run_dir, 0)["candidate_commit"]
    );
    let recheck_packet = read(&run_dir.join("actions/action-4/packet.json"));
    assert_eq!(
        recheck_packet["last_transition"]["reviewed_commit"],
        action_response(&run_dir, 2)["candidate_commit"]
    );
    let first = action_response(&run_dir, 1);
    let last = action_response(&run_dir, 5);
    assert_eq!(first["route"], "issues");
    assert_eq!(last["route"], "clean");
    let checkout = run_dir.join("worktree");
    assert_eq!(read(&checkout.join("simulation.json"))["attempt"], 2);
    assert_eq!(read(&checkout.join("simulation.json"))["issue"], false);
    assert_eq!(read(&checkout.join("repair.json"))["issue_observed"], true);
    assert_eq!(read(&checkout.join("engine.json"))["correct"], true);
    assert!(!repo.join("simulation.json").exists());
    assert_eq!(read(&repo.join("engine.json"))["correct"], false);
}

#[test]
fn clean_result_skips_repair_role() {
    let (_temp, repo, run_dir) = fixture("repairable", true);
    success(run(&repo, &run_dir));
    assert_eq!(status(&run_dir)["state"]["status"], "complete");
    assert_eq!(action_response(&run_dir, 1)["route"], "clean");
    assert!(!run_dir.join("actions/action-2").exists());
    assert!(!run_dir.join("worktree/repair.json").exists());
}

#[test]
fn repeated_unsuccessful_repairs_stop_at_the_frozen_transition_bound() {
    let (_temp, repo, run_dir) = fixture("permanent", false);
    let _output = run(&repo, &run_dir);
    let state = status(&run_dir);
    assert_eq!(state["state"]["status"], "stopped");
    assert!(state["state"]["stop_reason"]
        .as_str()
        .unwrap()
        .contains("transition"));
    assert_eq!(
        read(&run_dir.join("worktree/engine.json"))["correct"],
        false
    );
    assert!(!run_dir.join("actions/action-8").exists());
    let before = state.clone();
    let head_before = Command::new("git")
        .arg("-C")
        .arg(run_dir.join("worktree"))
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap()
        .stdout;
    let reset = cli(&[
        "reset",
        run_dir.to_str().unwrap(),
        "--acknowledge-external-effects",
    ]);
    assert!(!reset.status.success(), "exhausted budget cannot be reset");
    assert_eq!(status(&run_dir), before);
    let head_after = Command::new("git")
        .arg("-C")
        .arg(run_dir.join("worktree"))
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap()
        .stdout;
    assert_eq!(head_before, head_after);
    assert!(!run_dir.join("actions/action-8").exists());
}

#[test]
fn missing_or_undeclared_reviewer_route_cannot_dispatch_a_role_or_replay() {
    for mode in ["missing_route", "unauthorized_route"] {
        let (_temp, repo, run_dir) = fixture(mode, false);
        let output = run(&repo, &run_dir);
        assert!(
            !output.status.success(),
            "invalid route was accepted: {mode}"
        );
        let before = status(&run_dir);
        assert_eq!(before["state"]["status"], "running");
        assert_eq!(before["state"]["gate"], "reviewer");
        assert!(!run_dir.join("actions/action-2").exists());
        let resumed = cli(&["resume", run_dir.to_str().unwrap()]);
        assert!(
            !resumed.status.success(),
            "uncertain review was replayed: {mode}"
        );
        assert_eq!(status(&run_dir), before);
        assert!(!run_dir.join("actions/action-2").exists());
    }
}

#[test]
fn ignored_worker_file_prevents_review_and_is_not_deleted_by_reset() {
    let (_temp, repo, run_dir) = fixture("repairable", false);
    fs::write(repo.join(".gitignore"), "ignored-helper.tmp\n").unwrap();
    let action_path = repo.join("action.py");
    let code = fs::read_to_string(&action_path).unwrap().replace(
        "candidate = commit([\"simulation.json\"], \"Record simulated observation\")",
        "Path(\"ignored-helper.tmp\").write_text(\"unreviewed content\")\n        candidate = commit([\"simulation.json\"], \"Record simulated observation\")",
    );
    assert!(code.contains("unreviewed content"));
    fs::write(&action_path, code).unwrap();
    git(&repo, &["add", ".gitignore", "action.py"]);
    git(&repo, &["commit", "-qm", "fixture with ignored side file"]);
    let output = run(&repo, &run_dir);
    assert!(
        !output.status.success(),
        "ignored side file must prevent candidate acceptance"
    );
    let state = status(&run_dir);
    assert_eq!(state["state"]["status"], "running");
    assert_eq!(state["state"]["gate"], "worker");
    assert!(!run_dir.join("actions/action-1").exists());
    let ignored = run_dir.join("worktree/ignored-helper.tmp");
    assert_eq!(fs::read_to_string(&ignored).unwrap(), "unreviewed content");
    let resumed = cli(&["resume", run_dir.to_str().unwrap()]);
    assert!(
        !resumed.status.success(),
        "uncertain action must not replay"
    );
    let reset = cli(&[
        "reset",
        run_dir.to_str().unwrap(),
        "--acknowledge-external-effects",
    ]);
    assert!(
        !reset.status.success(),
        "reset must not remove ignored content"
    );
    assert_eq!(fs::read_to_string(&ignored).unwrap(), "unreviewed content");
    assert!(!repo.join("ignored-helper.tmp").exists());
}

#[test]
fn worker_can_choose_repair_then_rerun_through_the_cli() {
    let (_temp, repo, run_dir) = fixture_for("worker-dispatch", "repairable", false);
    success(run(&repo, &run_dir));
    assert_eq!(status(&run_dir)["state"]["status"], "complete");
    assert_eq!(action_response(&run_dir, 0)["route"], "issues");
    assert!(action_response(&run_dir, 0)["report"]
        .as_str()
        .unwrap()
        .contains("exited 7"));
    assert_eq!(
        read(&run_dir.join("actions/action-1/packet.json"))["candidate_route"],
        "issues"
    );
    assert_eq!(
        read(&run_dir.join("actions/action-2/packet.json"))["role"],
        "repair_specialist"
    );
    assert_eq!(action_response(&run_dir, 4)["route"], "clean");
    assert!(action_response(&run_dir, 4)["report"]
        .as_str()
        .unwrap()
        .contains("exited 0"));
}

#[test]
fn worker_selected_clean_route_skips_repair() {
    let (_temp, repo, run_dir) = fixture_for("worker-dispatch", "repairable", true);
    success(run(&repo, &run_dir));
    assert_eq!(status(&run_dir)["state"]["status"], "complete");
    assert_eq!(action_response(&run_dir, 0)["route"], "clean");
    assert!(!run_dir.join("actions/action-2").exists());
}

#[test]
fn worker_selected_repeated_failure_respects_transition_limit() {
    let (_temp, repo, run_dir) = fixture_for("worker-dispatch", "permanent", false);
    let output = run(&repo, &run_dir);
    assert!(!output.status.success());
    let state = status(&run_dir);
    assert_eq!(state["state"]["status"], "stopped");
    assert_eq!(
        state["state"]["stop_reason"],
        "workflow stage transition limit reached"
    );
    assert_eq!(state["state"]["stage_transitions"], 3);
    assert!(!run_dir.join("actions/action-8").exists());
}

#[test]
fn invalid_worker_route_never_dispatches_another_role() {
    for mode in ["missing_route", "unauthorized_route"] {
        let (_temp, repo, run_dir) = fixture_for("worker-dispatch", mode, false);
        let output = run(&repo, &run_dir);
        assert!(!output.status.success());
        let state = status(&run_dir);
        assert_eq!(state["state"]["status"], "running");
        assert_eq!(state["state"]["gate"], "worker");
        assert!(!run_dir.join("actions/action-1").exists());
        let resumed = cli(&["resume", run_dir.to_str().unwrap()]);
        assert!(!resumed.status.success());
        assert!(!run_dir.join("actions/action-1").exists());
    }
}

#[test]
fn reviewer_can_reject_the_workers_selected_route() {
    let (_temp, repo, run_dir) = fixture_for("worker-dispatch", "wrong_route", false);
    let output = run(&repo, &run_dir);
    assert!(!output.status.success());
    let state = status(&run_dir);
    assert_eq!(state["state"]["status"], "stopped");
    assert_eq!(state["state"]["stop_reason"], "review cycle limit reached");
    assert_eq!(action_response(&run_dir, 0)["route"], "clean");
    assert_eq!(action_response(&run_dir, 1)["outcome"], "changes_required");
    assert_eq!(action_response(&run_dir, 2)["route"], "clean");
    assert_eq!(action_response(&run_dir, 3)["outcome"], "changes_required");
    assert!(!run_dir.join("actions/action-4").exists());
}
