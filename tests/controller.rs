#![cfg(target_os = "macos")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn git(repo: &Path, args: &[&str]) {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir(&project).unwrap();
    for entry in fs::read_dir("examples/data-report").unwrap() {
        let e = entry.unwrap();
        if e.path().is_file() {
            fs::copy(e.path(), project.join(e.file_name())).unwrap();
        }
    }
    git(&project, &["init", "-q"]);
    git(&project, &["config", "user.name", "Test"]);
    git(&project, &["config", "user.email", "test@example.invalid"]);
    git(&project, &["add", "."]);
    git(&project, &["commit", "-qm", "baseline"]);
    let run = tmp.path().join("run");
    (tmp, project, run)
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn actual_two_stage_run_completes_and_reviews_exact_candidates() {
    let (_tmp, p, r) = setup();
    let o = cli(&[
        "run",
        p.join("workflow.toml").to_str().unwrap(),
        "--project",
        p.to_str().unwrap(),
        "--run-dir",
        r.to_str().unwrap(),
    ]);
    assert!(
        o.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    let s: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(s["state"]["status"], "complete");
    for id in 0..4 {
        assert!(r.join(format!("actions/action-{id}/packet.json")).exists());
        assert!(r.join(format!("actions/action-{id}/outcome.json")).exists());
    }
    assert!(!p.join("validation.json").exists());
}
#[test]
fn provider_failure_remains_uncertain_until_explicit_reset() {
    let (_tmp, p, r) = setup();
    fs::write(p.join("workflow_action.py"), "import sys\nsys.exit(7)\n").unwrap();
    git(&p, &["add", "workflow_action.py"]);
    git(&p, &["commit", "-qm", "fail"]);
    let run = cli(&[
        "run",
        p.join("workflow.toml").to_str().unwrap(),
        "--project",
        p.to_str().unwrap(),
        "--run-dir",
        r.to_str().unwrap(),
    ]);
    assert!(!run.status.success());
    let status = cli(&["status", r.to_str().unwrap()]);
    let s: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(s["state"]["status"], "running");
    let resume = cli(&["resume", r.to_str().unwrap()]);
    assert!(!resume.status.success());
    assert!(String::from_utf8_lossy(&resume.stderr).contains("uncertain"));
    assert!(!cli(&["reset", r.to_str().unwrap()]).status.success());
    assert!(cli(&[
        "reset",
        r.to_str().unwrap(),
        "--acknowledge-external-effects"
    ])
    .status
    .success());
    let status = cli(&["status", r.to_str().unwrap()]);
    let s: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(s["state"]["status"], "ready");
}
fn change_script(p: &Path, f: impl FnOnce(String) -> String) {
    let path = p.join("workflow_action.py");
    let old = fs::read_to_string(&path).unwrap();
    fs::write(&path, f(old)).unwrap();
    git(p, &["add", "workflow_action.py"]);
    git(p, &["commit", "-qm", "modify fixture"]);
}
fn run_cli(p: &Path, r: &Path) -> Output {
    cli(&[
        "run",
        p.join("workflow.toml").to_str().unwrap(),
        "--project",
        p.to_str().unwrap(),
        "--run-dir",
        r.to_str().unwrap(),
    ])
}
fn state(r: &Path) -> serde_json::Value {
    let output = cli(&["status", r.to_str().unwrap()]);
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn invalid_schema_output_never_starts_reviewer() {
    let (_tmp, p, r) = setup();
    change_script(&p, |s| {
        s.replace("return {\"count\": len(values)", "return {\"count\": -1")
    });
    let result = run_cli(&p, &r);
    assert!(
        !result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("violates schema"));
    assert_eq!(state(&r)["state"]["status"], "running");
    assert!(!r.join("actions/action-1").exists());
}
#[test]
fn wrong_identity_never_advances() {
    let (_tmp, p, r) = setup();
    change_script(&p, |s| {
        s.replace(
            "\"action_id\": packet[\"action_id\"]",
            "\"action_id\": \"forged\"",
        )
    });
    assert!(!run_cli(&p, &r).status.success());
    assert_eq!(state(&r)["state"]["status"], "running");
    assert!(!r.join("actions/action-1").exists());
}
#[test]
fn worker_unblock_retry_and_review_correction_are_routed_with_feedback() {
    let (_tmp, p, r) = setup();
    change_script(&p, |s| {
        let s=s.replace("if gate == \"unblocker\":\n        output(packet, \"blocked\", \"This deterministic fixture cannot repair invalid input automatically.\")", "if gate == \"unblocker\":\n        output(packet, \"retry\", \"Recovered test blocker.\")");
        let s=s.replace("if stage not in (\"validate\", \"report\"):", "if gate == \"worker\" and stage == \"validate\" and not packet[\"feedback\"]:\n        output(packet, \"blocked\", \"Missing temporary prerequisite.\")\n        return\n    if stage not in (\"validate\", \"report\"):");
        let s = s.replace(
            "json.dumps(expected, indent=2)",
            "json.dumps(expected, indent=4 if 'Correction required.' in packet['feedback'] else 2)",
        );
        s.replace("if actual != expected:", "if stage == 'validate' and 'Correction required.' not in packet['feedback']:\n            output(packet, 'changes_required', 'Correction required.', inspected=inspected)\n            return\n        if actual != expected:")
    });
    let result = run_cli(&p, &r);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let packet: serde_json::Value =
        serde_json::from_slice(&fs::read(r.join("actions/action-4/packet.json")).unwrap()).unwrap();
    assert_eq!(packet["gate"], "worker");
    assert!(packet["feedback"]
        .to_string()
        .contains("Correction required."));
    assert!(!packet["candidate_commit"].is_null());
    assert_eq!(state(&r)["state"]["status"], "complete");
}
#[test]
fn repeated_worker_block_stops_after_one_unblocker() {
    let (_tmp, p, r) = setup();
    change_script(&p, |s| {
        let s=s.replace("if gate == \"unblocker\":\n        output(packet, \"blocked\", \"This deterministic fixture cannot repair invalid input automatically.\")", "if gate == \"unblocker\":\n        output(packet, \"retry\", \"Try again.\")");
        s.replace("if stage not in (\"validate\", \"report\"):", "if gate == 'worker':\n        output(packet, 'blocked', 'Still blocked.')\n        return\n    if stage not in (\"validate\", \"report\"):")
    });
    assert!(!run_cli(&p, &r).status.success());
    let s = state(&r);
    assert_eq!(s["state"]["status"], "stopped");
    assert!(!r.join("actions/action-3").exists());
}
#[test]
fn killed_controller_never_replays_running_action() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let (_tmp, p, r) = setup();
    change_script(&p, |_| {
        "import os,time\nfrom pathlib import Path\np=Path(os.environ['SIMON_SCRATCH'])\n(p/'started').write_text('yes')\ntime.sleep(2)\n(p/'finished').write_text('yes')\n".into()
    });
    let mut child = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            p.join("workflow.toml").to_str().unwrap(),
            "--project",
            p.to_str().unwrap(),
            "--run-dir",
            r.to_str().unwrap(),
        ])
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    let marker = r.join("actions/action-0/scratch/started");
    while !marker.exists() {
        assert!(Instant::now() < deadline, "action did not start");
        thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let finished = r.join("actions/action-0/scratch/finished");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !finished.exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(30));
    }
    assert_eq!(state(&r)["state"]["status"], "running");
    assert!(!cli(&["resume", r.to_str().unwrap()]).status.success());
    assert!(!r.join("actions/action-1").exists());
}
#[test]
fn live_authority_or_state_mutation_is_detected_and_running_state_restored() {
    let (_tmp, p, r) = setup();
    change_script(&p, |_| {
        "import json,os\nfrom pathlib import Path\np=Path(os.environ['SIMON_PACKET']); packet=json.loads(p.read_text()); state=p.parents[2]/'state.json'; s=json.loads(state.read_text()); s['state']['status']='ready'; state.write_text(json.dumps(s)); print(json.dumps({'action_id':packet['action_id'],'outcome':'blocked','report':'x','candidate_commit':None,'inspected_commit':None}))\n".into()
    });
    let result = run_cli(&p, &r);
    assert!(!result.status.success());
    assert_eq!(state(&r)["state"]["status"], "running");
    assert!(String::from_utf8_lossy(&result.stderr).contains("failed with exit"));
    let stderr = fs::read_to_string(r.join("actions/action-0/stderr.txt")).unwrap();
    assert!(stderr.contains("PermissionError") || stderr.contains("Operation not permitted"));
    assert!(!cli(&["resume", r.to_str().unwrap()]).status.success());
}
#[test]
fn review_checkout_mutation_stops_without_accepting_candidate() {
    let (_tmp, p, r) = setup();
    change_script(&p, |s| {
        s.replace("if stage not in (\"validate\", \"report\"):", "if gate == 'reviewer':\n        Path('unexpected-review-file').write_text('changed')\n    if stage not in (\"validate\", \"report\"):")
    });
    let result = run_cli(&p, &r);
    assert!(!result.status.success());
    let s = state(&r);
    assert_eq!(s["state"]["status"], "running");
    assert_eq!(s["state"]["gate"], "reviewer");
    assert!(!s["state"]["candidate_commit"].is_null());
    assert!(r.join("actions/action-1/failure.json").exists());
}
#[test]
fn run_uses_snapshot_when_original_template_and_schema_change() {
    let (_tmp, p, r) = setup();
    simon_says::controller::initialize(&p.join("workflow.toml"), &p, &r).unwrap();
    fs::write(p.join("workflow.toml"), "invalid template").unwrap();
    fs::write(p.join("validation.schema.json"), "false").unwrap();
    let done = simon_says::controller::execute(&r).unwrap();
    assert_eq!(done.state.status, simon_says::routing::Status::Complete);
}
#[test]
fn pre_action_failure_is_durable_and_not_repeated_on_resume() {
    let (_tmp, p, r) = setup();
    git(&p, &["rm", "-q", "input.json"]);
    git(&p, &["commit", "-qm", "remove input"]);
    assert!(!run_cli(&p, &r).status.success());
    let s = state(&r);
    assert_eq!(s["state"]["status"], "preparing");
    assert_eq!(s["state"]["action_id"], "action-0");
    assert_eq!(s["next_action"], 1);
    assert!(s["state"]["stop_reason"]
        .as_str()
        .unwrap()
        .contains("input"));
    assert!(!cli(&["resume", r.to_str().unwrap()]).status.success());
    assert!(!r.join("actions/action-0").exists());
}
#[test]
fn action_timeout_is_durable_and_does_not_replay() {
    let (_tmp, p, r) = setup();
    let workflow = p.join("workflow.toml");
    let t = fs::read_to_string(&workflow).unwrap().replacen(
        "kind = \"command\"",
        "kind = \"command\"\ntimeout_seconds = 1",
        1,
    );
    fs::write(&workflow, t).unwrap();
    fs::write(
        p.join("workflow_action.py"),
        "import time\ntime.sleep(30)\n",
    )
    .unwrap();
    git(&p, &["add", "workflow.toml", "workflow_action.py"]);
    git(&p, &["commit", "-qm", "timeout fixture"]);
    let result = run_cli(&p, &r);
    assert!(!result.status.success());
    let s = state(&r);
    assert_eq!(s["state"]["status"], "running");
    assert!(s["state"]["stop_reason"]
        .as_str()
        .unwrap()
        .contains("timed out"));
    assert!(r.join("actions/action-0/process.json").exists());
    assert!(!cli(&["resume", r.to_str().unwrap()]).status.success());
}
#[test]
fn killed_during_review_checkout_requires_explicit_recovery() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let (_tmp, p, r) = setup();
    let marker = p.parent().unwrap().join("review-filter-started");
    let done = p.parent().unwrap().join("review-filter-done");
    let script = p.join("review-filter.sh");
    fs::write(&script, format!("#!/bin/sh\ncase \"$(pwd)\" in */reviews/*) printf started > '{}'; sleep 3; cat; printf done > '{}'; exit 0;; esac\ncat\n", marker.display(), done.display())).unwrap();
    fs::write(
        p.join(".gitattributes"),
        "input.json filter=reviewfixture\n",
    )
    .unwrap();
    git(
        &p,
        &[
            "config",
            "filter.reviewfixture.smudge",
            &format!("sh {}", script.display()),
        ],
    );
    git(&p, &["add", ".gitattributes", "review-filter.sh"]);
    git(&p, &["commit", "-qm", "review filter"]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            p.join("workflow.toml").to_str().unwrap(),
            "--project",
            p.to_str().unwrap(),
            "--run-dir",
            r.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(25);
    while !marker.exists() {
        assert!(Instant::now() < deadline, "review filter did not start");
        thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !done.exists() {
        assert!(Instant::now() < deadline, "orphan checkout did not finish");
        thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(Duration::from_millis(500));
    let s = state(&r);
    assert_eq!(s["state"]["status"], "preparing");
    assert_eq!(s["state"]["action_id"], "action-1");
    assert_eq!(s["next_action"], 2);
    let result = cli(&["resume", r.to_str().unwrap()]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("preparation is uncertain"));
    assert!(!cli(&["reset", r.to_str().unwrap()]).status.success());
    let orphan = r.join("reviews/action-1/worktree");
    let orphan_exists = orphan.exists();
    // Git may remove its incomplete checkout after the killed parent closes pipes.
    // Recovery must not remove anything that remains in the review directory.
    let evidence = r.join("reviews/action-1/preserve-evidence");
    fs::write(&evidence, "keep").unwrap();
    assert!(cli(&[
        "reset",
        r.to_str().unwrap(),
        "--acknowledge-external-effects"
    ])
    .status
    .success());
    if orphan_exists {
        assert!(orphan.exists(), "recovery removed orphan checkout");
    }
    assert_eq!(fs::read_to_string(evidence).unwrap(), "keep");
    assert_eq!(state(&r)["state"]["status"], "ready");
}
