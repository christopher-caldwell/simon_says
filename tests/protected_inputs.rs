#![cfg(target_os = "macos")]

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn git(repo: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(args)
        .output()
        .unwrap()
}
fn setup(protect: bool) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/dynamic-repair").unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
    }
    fs::write(repo.join("mode.json"), "{\"mode\":\"repairable\"}\n").unwrap();
    fs::write(repo.join("engine.json"), "{\"correct\":false}\n").unwrap();
    let action = repo.join("action.py");
    let text = fs::read_to_string(&action).unwrap().replace(
        "observation = read(\"simulation.json\")\n        if not observation[\"issue\"]:",
        "observation = read(\"simulation.json\")\n        observation[\"tampered\"] = True\n        Path(\"simulation.json\").write_text(json.dumps(observation) + \"\\n\")\n        subprocess.run([\"git\", \"add\", \"--\", \"simulation.json\"], check=True)\n        if not observation[\"issue\"]:",
    );
    assert!(text.contains("observation[\"tampered\"] = True"));
    fs::write(action, text).unwrap();
    if protect {
        let path = repo.join("workflow.toml");
        let text = fs::read_to_string(&path).unwrap().replace(
            "inputs = [\"simulation.json\", \"engine.json\", \"mode.json\"]",
            "inputs = [\"simulation.json\", \"engine.json\", \"mode.json\"]\nprotected_inputs = [\"simulation.json\"]",
        );
        assert!(text.contains("protected_inputs"));
        fs::write(path, text).unwrap();
    }
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = tmp.path().join("run");
    (tmp, repo, run)
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
fn json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn stage_protects_prior_reviewed_output_and_stops_before_repair_review() {
    let (_tmp, repo, run_dir) = setup(true);
    let output = run(&repo, &run_dir);
    assert!(
        !output.status.success(),
        "a candidate changing a protected input must be rejected"
    );
    let state = json(&run_dir.join("state.json"));
    assert_eq!(state["state"]["status"], "running");
    assert_eq!(state["state"]["stage"], "repair");
    assert_eq!(state["state"]["gate"], "worker");
    assert_eq!(
        json(&run_dir.join("actions/action-2/packet.json"))["protected_inputs"],
        serde_json::json!(["simulation.json"])
    );
    assert_eq!(
        json(&run_dir.join("worktree/simulation.json"))["tampered"],
        true
    );
    assert!(!run_dir.join("actions/action-3").exists());
    let resumed = cli(&["resume", run_dir.to_str().unwrap()]);
    assert!(
        !resumed.status.success(),
        "uncertain action must never replay"
    );
}

#[test]
fn inputs_remain_editable_by_default_for_workflows_that_intend_it() {
    let (_tmp, repo, run_dir) = setup(false);
    let output = run(&repo, &run_dir);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let outcome = json(&run_dir.join("actions/action-2/outcome.json"));
    let response: Value = serde_json::from_str(outcome["response"].as_str().unwrap()).unwrap();
    let commit = response["candidate_commit"].as_str().unwrap();
    let spec = format!("{commit}:simulation.json");
    let content = git(&run_dir.join("worktree"), &["show", &spec]);
    let recorded: Value = serde_json::from_slice(&content.stdout).unwrap();
    assert_eq!(recorded["tampered"], true);
    assert_eq!(
        json(&run_dir.join("state.json"))["state"]["status"],
        "complete"
    );
}

#[test]
fn validation_rejects_protection_of_noninputs_outputs_or_duplicate_entries() {
    let (_tmp, repo, _run_dir) = setup(false);
    let path = repo.join("workflow.toml");
    let baseline = fs::read_to_string(&path).unwrap();
    let pattern = "inputs = [\"simulation.json\", \"engine.json\", \"mode.json\"]";
    for extra in [
        "protected_inputs = [\"missing.json\"]",
        "protected_inputs = [\"simulation.json\", \"simulation.json\"]",
        "protected_inputs = [\"repair.json\"]\ninputs = [\"repair.json\"]",
    ] {
        let text = if extra.contains("inputs = [\"repair.json\"]") {
            baseline.replace(
                pattern,
                "inputs = [\"repair.json\"]\nprotected_inputs = [\"repair.json\"]",
            )
        } else {
            baseline.replace(pattern, &format!("{pattern}\n{extra}"))
        };
        fs::write(&path, text).unwrap();
        let output = cli(&["validate", path.to_str().unwrap()]);
        assert!(
            !output.status.success(),
            "invalid protected inputs accepted: {extra}"
        );
    }
}
