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
#[test]
fn original_context_and_purpose_are_frozen_across_all_role_handoffs() {
    let (tmp, p, r) = setup();
    change_script(&p, |s| {
        let s=s.replace("if gate == \"unblocker\":\n        output(packet, \"blocked\", \"This deterministic fixture cannot repair invalid input automatically.\")", "if gate == \"unblocker\":\n        output(packet, \"retry\", \"Recovered test blocker.\")");
        let s=s.replace("if stage not in (\"validate\", \"report\"):", "if gate == 'worker' and stage == 'validate' and not packet['feedback']:\n        output(packet, 'blocked', 'Temporary prerequisite.')\n        return\n    if stage not in (\"validate\", \"report\"):");
        let s = s.replace(
            "json.dumps(expected, indent=2)",
            "json.dumps(expected, indent=4 if 'Correction required.' in packet['feedback'] else 2)",
        );
        s.replace("if actual != expected:", "if stage == 'validate' and 'Correction required.' not in packet['feedback']:\n            output(packet, 'changes_required', 'Correction required.', inspected=inspected)\n            return\n        if actual != expected:")
    });
    let wf = p.join("workflow.toml");
    let text = fs::read_to_string(&wf).unwrap().replacen(
        "schema_version = 1",
        "schema_version = 1\npurpose = \"Validate data without losing the original request.\"",
        1,
    );
    fs::write(&wf, text).unwrap();
    git(&p, &["add", "workflow.toml"]);
    git(&p, &["commit", "-qm", "add purpose"]);
    let context = tmp.path().join("original-task.txt");
    fs::write(
        &context,
        "Original user request: validate every measurement; do not lose feedback.",
    )
    .unwrap();
    simon_says::controller::initialize_with_context(&wf, &p, &r, Some(&context)).unwrap();
    fs::write(&context, "later changed text").unwrap();
    let done = simon_says::controller::execute(&r).unwrap();
    assert_eq!(done.state.status, simon_says::routing::Status::Complete);
    let mut gates = std::collections::BTreeSet::new();
    for item in fs::read_dir(r.join("actions")).unwrap() {
        let packet: serde_json::Value =
            serde_json::from_slice(&fs::read(item.unwrap().path().join("packet.json")).unwrap())
                .unwrap();
        gates.insert(packet["gate"].as_str().unwrap().to_owned());
        assert_eq!(
            packet["run_context"],
            "Original user request: validate every measurement; do not lose feedback."
        );
        assert_eq!(
            packet["workflow_purpose"],
            "Validate data without losing the original request."
        );
    }
    assert_eq!(
        gates,
        ["reviewer", "unblocker", "worker"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    );
}
#[test]
fn run_cli_accepts_and_records_original_context() {
    let (tmp, p, r) = setup();
    let context = tmp.path().join("task.txt");
    fs::write(&context, "The user's specific task.").unwrap();
    let result = cli(&[
        "run",
        p.join("workflow.toml").to_str().unwrap(),
        "--project",
        p.to_str().unwrap(),
        "--run-dir",
        r.to_str().unwrap(),
        "--context",
        context.to_str().unwrap(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let packet: serde_json::Value =
        serde_json::from_slice(&fs::read(r.join("actions/action-1/packet.json")).unwrap()).unwrap();
    assert_eq!(packet["run_context"], "The user's specific task.");
}
#[test]
fn invalid_context_is_rejected_before_run_directory_creation() {
    let (tmp, p, r) = setup();
    let context = tmp.path().join("huge.txt");
    fs::write(&context, vec![b'x'; 32769]).unwrap();
    let result = cli(&[
        "run",
        p.join("workflow.toml").to_str().unwrap(),
        "--project",
        p.to_str().unwrap(),
        "--run-dir",
        r.to_str().unwrap(),
        "--context",
        context.to_str().unwrap(),
    ]);
    assert!(!result.status.success());
    assert!(!r.exists());
}
#[test]
fn explicit_approval_binds_checkpoint_and_context_then_continues() {
    let (tmp, p, r) = setup();
    let workflow = p.join("workflow.toml");
    let source = fs::read_to_string(&workflow).unwrap().replacen(
        "on_pass = \"report\"",
        "on_pass = \"report\"\napproval_required = true",
        1,
    );
    fs::write(&workflow, source).unwrap();
    git(&p, &["add", "workflow.toml"]);
    git(&p, &["commit", "-qm", "require approval"]);
    let context = tmp.path().join("task.txt");
    fs::write(
        &context,
        "Do not finalize or retire evidence until I approve.",
    )
    .unwrap();
    let result = cli(&[
        "run",
        workflow.to_str().unwrap(),
        "--project",
        p.to_str().unwrap(),
        "--run-dir",
        r.to_str().unwrap(),
        "--context",
        context.to_str().unwrap(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let s = state(&r);
    assert_eq!(s["state"]["status"], "awaiting_approval");
    assert_eq!(s["state"]["stage"], "validate");
    assert!(!r.join("actions/action-2").exists());
    assert!(!cli(&["resume", r.to_str().unwrap()]).status.success());
    let commit = s["state"]["accepted_commit"].as_str().unwrap();
    let digest = s["snapshot_digest"].as_str().unwrap();
    let approval = s["state"]["pending_approval"]["approval_id"]
        .as_str()
        .unwrap();
    assert!(!cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        "wrong",
        "--expected-approval",
        approval
    ])
    .status
    .success());
    assert!(!cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        "wrong",
        "--expected-digest",
        digest,
        "--expected-approval",
        approval
    ])
    .status
    .success());
    assert_eq!(state(&r)["state"]["status"], "awaiting_approval");
    let approved = cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        approval,
        "--note",
        "Reviewed findings",
    ]);
    assert!(
        approved.status.success(),
        "{}",
        String::from_utf8_lossy(&approved.stderr)
    );
    let done = state(&r);
    assert_eq!(done["state"]["status"], "complete");
    assert_eq!(done["state"]["approvals"][0]["stage"], "validate");
    assert_eq!(done["state"]["approvals"][0]["commit"], commit);
    assert_eq!(done["state"]["approvals"][0]["snapshot_digest"], digest);
    assert_eq!(done["state"]["approvals"][0]["note"], "Reviewed findings");
    assert!(!cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        approval
    ])
    .status
    .success());
}
#[test]
fn approval_refuses_dirty_or_replaced_checkout() {
    let (_tmp, p, r) = setup();
    let workflow = p.join("workflow.toml");
    let source = fs::read_to_string(&workflow).unwrap().replacen(
        "on_pass = \"report\"",
        "on_pass = \"report\"\napproval_required = true",
        1,
    );
    fs::write(&workflow, source).unwrap();
    git(&p, &["add", "workflow.toml"]);
    git(&p, &["commit", "-qm", "approval"]);
    assert!(run_cli(&p, &r).status.success());
    let s = state(&r);
    let commit = s["state"]["accepted_commit"].as_str().unwrap();
    let digest = s["snapshot_digest"].as_str().unwrap();
    let approval = s["state"]["pending_approval"]["approval_id"]
        .as_str()
        .unwrap();
    fs::write(r.join("worktree/input.json"), "changed").unwrap();
    assert!(!cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        approval
    ])
    .status
    .success());
    assert_eq!(state(&r)["state"]["status"], "approval_checking");
    assert!(!cli(&["resume", r.to_str().unwrap()]).status.success());
    assert!(!cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        approval
    ])
    .status
    .success());
    assert!(cli(&[
        "reset",
        r.to_str().unwrap(),
        "--acknowledge-external-effects"
    ])
    .status
    .success());
    assert_eq!(state(&r)["state"]["status"], "awaiting_approval");
    assert!(cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        approval
    ])
    .status
    .success());
}
#[test]
fn claude_worker_nested_sandbox_failure_stops_before_invocation() {
    let (_tmp, p, r) = setup();
    let workflow = p.join("workflow.toml");
    let source = fs::read_to_string(&workflow).unwrap().replace(
        "[roles.worker]\nkind = \"command\"\nargv = [\"python3\", \"workflow_action.py\"]",
        "[roles.worker]\nkind = \"agent\"\nadapter = \"claude\"\nprofile = \"worker-profile.md\"",
    );
    fs::write(&workflow, source).unwrap();
    fs::write(p.join("worker-profile.md"), "Work profile").unwrap();
    git(&p, &["add", "workflow.toml", "worker-profile.md"]);
    git(&p, &["commit", "-qm", "claude worker"]);
    let result = run_cli(&p, &r);
    assert!(!result.status.success());
    let s = state(&r);
    assert_eq!(s["state"]["status"], "preparing");
    assert!(!r.join("actions/action-0/invocation.json").exists());
    assert!(!r.join("actions/action-0/stdout.txt").exists());
    assert!(String::from_utf8_lossy(&result.stderr)
        .to_ascii_lowercase()
        .contains("sandbox"));
}
#[test]
fn approval_challenge_cannot_be_replayed_for_same_commit_in_later_stage() {
    let (_tmp, p, r) = setup();
    change_script(&p, |s| {
        s.replace("if stage not in (\"validate\", \"report\"):", "if gate == 'worker' and stage == 'report':\n        output(packet, 'complete', 'Use the already committed report.', candidate=git('rev-parse','HEAD'))\n        return\n    if stage not in (\"validate\", \"report\"):")
    });
    fs::write(
        p.join("summary.json"),
        r#"{"summary":"3 measurements; range 4 to 9.","mean":7.0}"#,
    )
    .unwrap();
    let workflow = p.join("workflow.toml");
    let source = fs::read_to_string(&workflow).unwrap().replace(
        "max_review_cycles = 3",
        "approval_required = true\nmax_review_cycles = 3",
    );
    fs::write(&workflow, source).unwrap();
    git(&p, &["add", "workflow.toml", "summary.json"]);
    git(&p, &["commit", "-qm", "two approval stages"]);
    assert!(run_cli(&p, &r).status.success());
    let first = state(&r);
    let commit = first["state"]["accepted_commit"].as_str().unwrap();
    let digest = first["snapshot_digest"].as_str().unwrap();
    let old = first["state"]["pending_approval"]["approval_id"]
        .as_str()
        .unwrap();
    let first_approve = cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        old,
    ]);
    assert!(
        first_approve.status.success(),
        "{}",
        String::from_utf8_lossy(&first_approve.stderr)
    );
    let second = state(&r);
    assert_eq!(second["state"]["status"], "awaiting_approval");
    assert_eq!(second["state"]["stage"], "report");
    assert_eq!(second["state"]["accepted_commit"], commit);
    let new = second["state"]["pending_approval"]["approval_id"]
        .as_str()
        .unwrap();
    assert_ne!(old, new);
    assert!(!cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        old
    ])
    .status
    .success());
    assert_eq!(state(&r)["state"]["status"], "awaiting_approval");
    assert!(cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        new
    ])
    .status
    .success());
    assert_eq!(state(&r)["state"]["status"], "complete");
}
#[test]
fn interrupted_approval_filter_requires_explicit_recovery() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let (tmp, p, r) = setup();
    let enable = tmp.path().join("enable-filter");
    let marker = tmp.path().join("approval-filter-started");
    let done = tmp.path().join("approval-filter-done");
    let script = p.join("approval-filter.sh");
    fs::write(&script,format!("#!/bin/sh\nif [ -f '{}' ]; then printf started > '{}'; sleep 3; cat; printf done > '{}'; else cat; fi\n",enable.display(),marker.display(),done.display())).unwrap();
    fs::write(
        p.join(".gitattributes"),
        "input.json filter=approvalfixture\n",
    )
    .unwrap();
    git(
        &p,
        &[
            "config",
            "filter.approvalfixture.clean",
            &format!("sh {}", script.display()),
        ],
    );
    let wf = p.join("workflow.toml");
    let source = fs::read_to_string(&wf).unwrap().replacen(
        "on_pass = \"report\"",
        "on_pass = \"report\"\napproval_required = true",
        1,
    );
    fs::write(&wf, source).unwrap();
    git(
        &p,
        &[
            "add",
            ".gitattributes",
            "approval-filter.sh",
            "workflow.toml",
        ],
    );
    git(&p, &["commit", "-qm", "approval filter"]);
    assert!(run_cli(&p, &r).status.success());
    let s = state(&r);
    let commit = s["state"]["accepted_commit"].as_str().unwrap();
    let digest = s["snapshot_digest"].as_str().unwrap();
    let challenge = s["state"]["pending_approval"]["approval_id"]
        .as_str()
        .unwrap();
    fs::write(&enable, "yes").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "approve",
            r.to_str().unwrap(),
            "--expected-commit",
            commit,
            "--expected-digest",
            digest,
            "--expected-approval",
            challenge,
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() {
        assert!(Instant::now() < deadline, "approval filter did not start");
        thread::sleep(Duration::from_millis(20));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done.exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(state(&r)["state"]["status"], "approval_checking");
    assert!(!cli(&["resume", r.to_str().unwrap()]).status.success());
    assert!(!cli(&[
        "approve",
        r.to_str().unwrap(),
        "--expected-commit",
        commit,
        "--expected-digest",
        digest,
        "--expected-approval",
        challenge
    ])
    .status
    .success());
    fs::remove_file(enable).unwrap();
    assert!(cli(&[
        "reset",
        r.to_str().unwrap(),
        "--acknowledge-external-effects"
    ])
    .status
    .success());
    assert_eq!(state(&r)["state"]["status"], "awaiting_approval");
}
#[test]
fn reset_refuses_invalid_pending_approval_before_discarding_changes() {
    let (_tmp, p, r) = setup();
    let workflow = p.join("workflow.toml");
    let source = fs::read_to_string(&workflow).unwrap().replacen(
        "on_pass = \"report\"",
        "on_pass = \"report\"\napproval_required = true",
        1,
    );
    fs::write(&workflow, source).unwrap();
    git(&p, &["add", "workflow.toml"]);
    git(&p, &["commit", "-qm", "approval"]);
    assert!(run_cli(&p, &r).status.success());
    let state_path = r.join("state.json");
    let mut saved: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).unwrap()).unwrap();
    saved["state"]["status"] = "approval_checking".into();
    saved["state"]["pending_approval"] = serde_json::Value::Null;
    fs::write(&state_path, serde_json::to_vec_pretty(&saved).unwrap()).unwrap();
    let changed = r.join("worktree/input.json");
    fs::write(&changed, "must be preserved").unwrap();
    assert!(!cli(&[
        "reset",
        r.to_str().unwrap(),
        "--acknowledge-external-effects"
    ])
    .status
    .success());
    assert_eq!(fs::read_to_string(changed).unwrap(), "must be preserved");
}
