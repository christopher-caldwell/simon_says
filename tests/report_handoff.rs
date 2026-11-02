#![cfg(target_os = "macos")]
use serde_json::Value;
use std::{fs, path::Path, process::Command};

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

#[test]
fn report_only_roles_handoff_a_structured_issue_without_files_or_commits() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let run = temp.path().join("run");
    fs::create_dir(&repo).unwrap();
    fs::write(repo.join("instructions.md"), "Follow the role.\n").unwrap();
    fs::write(
        repo.join("workflow.toml"),
        r#"schema_version = 1
name = "report-handoff"
start = "observe"
max_stage_transitions = 3
[roles.actor]
kind = "command"
argv = ["python3", "-B", "role.py"]
[stages.observe]
worker = "actor"
instructions = "instructions.md"
on_pass = "complete"
[stages.observe.routes]
issue = "fix"
clean = "complete"
[stages.fix]
worker = "actor"
instructions = "instructions.md"
on_pass = "complete"
"#,
    )
    .unwrap();
    fs::write(repo.join("role.py"), r#"import json, os
p = json.load(open(os.environ['SIMON_PACKET']))
if p['stage'] == 'observe':
    response = {'action_id': p['action_id'], 'outcome': 'complete', 'report': {'error': 'bad data', 'code': 7}, 'route': 'issue'}
else:
    assert p['last_transition']['selected_route'] == 'issue'
    assert json.loads(p['last_transition']['report']) == {'error': 'bad data', 'code': 7}
    response = {'action_id': p['action_id'], 'outcome': 'complete', 'report': 'handled'}
print(json.dumps(response))
"#).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let output = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state: Value = serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap();
    assert_eq!(state["state"]["status"], "complete");
    assert_eq!(state["state"]["last_transition"]["report"], "handled");
    assert_eq!(
        state["state"]["accepted_commit"],
        state["worktree"]["baseline"]
    );
    assert!(!run.join("actions/action-2").exists());

    // New nonignored role work is checkpointed even without a declared output.
    let script = repo.join("role.py");
    let source = fs::read_to_string(&script).unwrap();
    fs::write(
        &script,
        source.replace(
            "p = json.load",
            "open('untracked.txt', 'w').write('unexpected')\np = json.load",
        ),
    )
    .unwrap();
    git(&repo, &["add", "role.py"]);
    git(&repo, &["commit", "-qm", "fixture leaves untracked file"]);
    let dirty_run = temp.path().join("dirty-run");
    let completed = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            dirty_run.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    assert!(dirty_run.join("actions/action-1").exists());
    assert!(dirty_run.join("worktree/untracked.txt").exists());
    assert!(!repo.join("untracked.txt").exists());

    // A completed report can be adopted after preserving an ignored file,
    // without replaying the already executed role.
    fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    fs::write(&script, source.replace("if p['stage'] == 'observe':", "if p['stage'] == 'observe':\n    os.makedirs('target', exist_ok=True)\n    open('target/evidence', 'w').write('preserve')")).unwrap();
    git(&repo, &["add", ".gitignore", "role.py"]);
    git(&repo, &["commit", "-qm", "fixture leaves ignored evidence"]);
    let adopt_run = temp.path().join("adopt-run");
    let stopped = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            adopt_run.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!stopped.status.success());
    fs::rename(
        adopt_run.join("worktree/target"),
        adopt_run.join("actions/action-0/scratch/preserved"),
    )
    .unwrap();
    let saved: Value =
        serde_json::from_slice(&fs::read(adopt_run.join("state.json")).unwrap()).unwrap();
    let candidate = saved["worktree"]["baseline"].as_str().unwrap();
    let adopted = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "adopt-worker",
            adopt_run.to_str().unwrap(),
            "--expected-action",
            "action-0",
            "--expected-candidate",
            candidate,
            "--acknowledge-external-effects",
        ])
        .output()
        .unwrap();
    assert!(
        adopted.status.success(),
        "{}",
        String::from_utf8_lossy(&adopted.stderr)
    );
    let resumed = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(["resume", adopt_run.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert!(!adopt_run.join("actions/action-2").exists());
}

#[test]
fn reviewed_handoff_passes_only_the_accepted_worker_report() {
    // Covered by the routing state machine, including a rejected candidate.
    // The full subprocess test above exercises the packet actually seen by the next role.
    let t = fs::read_to_string("examples/basic/workflow.toml").unwrap();
    let (w, _) = simon_says::Workflow::parse_and_validate(&t, Path::new("examples/basic")).unwrap();
    let mut state = simon_says::routing::State::new(&w, "base".into());
    state.start("worker".into()).unwrap();
    state
        .apply(
            &w,
            simon_says::routing::Response {
                action_id: "worker".into(),
                outcome: "complete".into(),
                report: "role result".into(),
                candidate_commit: Some("candidate".into()),
                inspected_commit: None,
                route: None,
            },
        )
        .unwrap();
    // The candidate report remains durable if Simon restarts before review.
    let saved = serde_json::to_vec(&state).unwrap();
    let mut state: simon_says::routing::State = serde_json::from_slice(&saved).unwrap();
    state.start("review".into()).unwrap();
    state
        .apply(
            &w,
            simon_says::routing::Response {
                action_id: "review".into(),
                outcome: "pass".into(),
                report: "reviewer says yes".into(),
                candidate_commit: None,
                inspected_commit: Some("candidate".into()),
                route: None,
            },
        )
        .unwrap();
    let transition = state.last_transition.unwrap();
    assert_eq!(transition.report.as_deref(), Some("role result"));
    assert_eq!(
        transition.review_report.as_deref(),
        Some("reviewer says yes")
    );
}

#[test]
fn report_only_worker_can_use_an_explicit_unblocker() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let run = temp.path().join("run");
    fs::create_dir(&repo).unwrap();
    fs::write(repo.join("instructions.md"), "Fixture\n").unwrap();
    fs::write(
        repo.join("workflow.toml"),
        r#"schema_version = 1
name = "report-unblock"
start = "work"
[roles.actor]
kind = "command"
argv = ["python3", "-B", "role.py"]
[stages.work]
worker = "actor"
unblocker = "actor"
instructions = "instructions.md"
on_pass = "complete"
"#,
    )
    .unwrap();
    fs::write(
        repo.join("role.py"),
        r#"import json, os
p = json.load(open(os.environ['SIMON_PACKET']))
if p['gate'] == 'unblocker':
    assert 'report_only' not in p and 'direct_handoff' not in p
    result = 'retry'
elif not p['feedback']:
    assert p['report_only']
    result = 'blocked'
else:
    result = 'complete'
print(json.dumps({'action_id': p['action_id'], 'outcome': result, 'report': result}))
"#,
    )
    .unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let output = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let state: Value = serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap();
    assert_eq!(state["state"]["status"], "complete");
    assert!(!run.join("actions/action-3").exists());
}
