#![cfg(target_os = "macos")]

use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const FIRST: &str = "11111111-1111-4111-8111-111111111111";
const SECOND: &str = "22222222-2222-4222-8222-222222222222";
const FAKE_CODEX: &str = r##"#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys
p = json.loads(pathlib.Path(os.environ['SIMON_PACKET']).read_text())
_ = sys.stdin.read()
worker = p['gate'] == 'worker'
expected = '11111111-1111-4111-8111-111111111111' if p['stage'] == 'first' else '22222222-2222-4222-8222-222222222222'
if 'resume' in sys.argv:
    idx = sys.argv.index('resume')
    if not worker or sys.argv[idx + 1] != expected:
        raise RuntimeError('wrong session was resumed')
    session = '44444444-4444-4444-8444-444444444444' if os.environ.get('FAKE_MISMATCH') == '1' else expected
else:
    session = expected if worker else '33333333-3333-4333-8333-333333333333'
if not (worker and os.environ.get('FAKE_NO_SESSION') == '1' and not p['feedback']):
    print(json.dumps({'type': 'thread.started', 'thread_id': session}))
if worker and os.environ.get('FAKE_BLOCK') == '1' and not p['feedback']:
    response = {'action_id': p['action_id'], 'outcome': 'blocked', 'report':'blocked once', 'candidate_commit':None, 'inspected_commit':None}
    print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':json.dumps(response)}}))
    print(json.dumps({'type':'turn.completed'}))
    raise SystemExit(0)
if worker:
    output = 'one.json' if p['stage'] == 'first' else 'two.json'
    if p['stage'] == 'first':
        prior = json.loads(pathlib.Path(output).read_text())['round'] if pathlib.Path(output).exists() else 0
        data = {'round': prior + 1}
    else:
        data = {'done': True}
    pathlib.Path(output).write_text(json.dumps(data) + '\n')
    subprocess.run(['git','add','--',output], check=True)
    subprocess.run(['git','-c','user.name=Fake','-c','user.email=fake@example.invalid','commit','-qm','candidate'], check=True)
    head = subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip()
    response = {'action_id': p['action_id'], 'outcome': 'complete', 'report':'candidate', 'candidate_commit':head, 'inspected_commit':None}
else:
    head = subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip()
    first_round = p['stage'] == 'first' and json.loads(pathlib.Path('one.json').read_text())['round'] == 1
    response = {'action_id': p['action_id'], 'outcome':'changes_required' if first_round else 'pass', 'report':'fix once' if first_round else 'passed', 'candidate_commit':None, 'inspected_commit':head}
    if p['stage'] == 'first' and p.get('review_routes'):
        response['outcome'] = 'pass'
        response['route'] = 'again' if first_round else 'next'
print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':json.dumps(response)}}))
if os.environ.get('FAKE_TERMINAL_FAILURE') == '1':
    print(json.dumps({'type':'turn.failed','error':{'message':'fixture failure'}}))
elif os.environ.get('FAKE_MISSING_TERMINAL') != '1':
    print(json.dumps({'type':'turn.completed'}))
"##;

fn git(repo: &Path, args: &[&str]) {
    let o = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
fn fixture(max_reviews: u32, enabled: bool) -> (tempfile::TempDir, PathBuf, PathBuf, String) {
    let t = tempfile::tempdir().unwrap();
    let repo = t.path().join("repo");
    let bin = t.path().join("bin");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&bin).unwrap();
    let fake = bin.join("codex");
    fs::write(&fake, FAKE_CODEX).unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(repo.join("input.json"), "{}\n").unwrap();
    fs::write(repo.join("profile.md"), "Fixture profile\n").unwrap();
    fs::write(repo.join("instructions.md"), "Fixture instructions\n").unwrap();
    fs::write(repo.join("stop.py"), "import json,os\np=json.load(open(os.environ['SIMON_PACKET'])); print(json.dumps({'action_id':p['action_id'],'outcome':'blocked','report':'stop','candidate_commit':None,'inspected_commit':None}))\n").unwrap();
    let workflow = format!(
        r#"schema_version = 1
name = "session-fixture"
start = "first"
[roles.worker]
kind = "agent"
adapter = "codex"
model = "fixture"
effort = "low"
profile = "profile.md"
resume_worker_session = {enabled}
[roles.review]
kind = "agent"
adapter = "codex"
model = "fixture"
effort = "low"
profile = "profile.md"
resume_worker_session = true
[roles.stop]
kind = "command"
argv = ["python3", "-B", "stop.py"]
[stages.first]
worker = "worker"
reviewer = "review"
unblocker = "stop"
instructions = "instructions.md"
inputs = ["input.json"]
outputs = ["one.json"]
on_pass = "second"
max_review_cycles = {max_reviews}
[stages.second]
worker = "worker"
reviewer = "review"
unblocker = "stop"
instructions = "instructions.md"
inputs = ["one.json"]
outputs = ["two.json"]
on_pass = "complete"
max_review_cycles = 2
"#
    );
    fs::write(repo.join("workflow.toml"), workflow).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let run = t.path().join("run");
    (t, repo, run, path)
}
fn cli(args: &[&str], path: &str, mismatch: bool) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_simon-says"));
    cmd.args(args).env("PATH", path);
    if mismatch {
        cmd.env("FAKE_MISMATCH", "1");
    }
    cmd.output().unwrap()
}
fn run(repo: &Path, run: &Path, path: &str, mismatch: bool) -> Output {
    cli(
        &[
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ],
        path,
        mismatch,
    )
}
fn state(run: &Path) -> Value {
    serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap()
}
fn plan(run: &Path, n: usize) -> Value {
    serde_json::from_slice(
        &fs::read(run.join(format!("actions/action-{n}/invocation.json"))).unwrap(),
    )
    .unwrap()
}

#[test]
fn failed_native_turn_never_dispatches_reviewer_or_replays() {
    let (_temp, repo, run_dir, path) = fixture(2, false);
    let output = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run_dir.to_str().unwrap(),
        ])
        .env("PATH", &path)
        .env("FAKE_TERMINAL_FAILURE", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(state(&run_dir)["state"]["status"], "running");
    assert!(!run_dir.join("actions/action-1").exists());
    assert!(!cli(&["resume", run_dir.to_str().unwrap()], &path, false)
        .status
        .success());
    assert!(!run_dir.join("actions/action-1").exists());
}

#[test]
fn incomplete_native_turn_never_dispatches_reviewer_or_replays() {
    let (_temp, repo, run_dir, path) = fixture(2, false);
    let output = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run_dir.to_str().unwrap(),
        ])
        .env("PATH", &path)
        .env("FAKE_MISSING_TERMINAL", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(state(&run_dir)["state"]["status"], "running");
    assert!(!run_dir.join("actions/action-1").exists());
    assert!(!cli(&["resume", run_dir.to_str().unwrap()], &path, false)
        .status
        .success());
    assert!(!run_dir.join("actions/action-1").exists());
}

#[test]
fn opt_in_reuses_only_same_stage_worker_and_keeps_reviewers_and_next_stage_fresh() {
    let (_t, repo, run_dir, path) = fixture(2, true);
    let out = run(&repo, &run_dir, &path, false);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state(&run_dir)["state"]["status"], "complete");
    assert!(plan(&run_dir, 0)["session_in"].is_null());
    assert_eq!(plan(&run_dir, 2)["session_in"], FIRST);
    for i in [1, 3, 4, 5] {
        assert!(plan(&run_dir, i)["session_in"].is_null(), "action {i}");
    }
    assert_eq!(state(&run_dir)["next_action"], 6);
    let raw: Value =
        serde_json::from_slice(&fs::read(run_dir.join("actions/action-4/outcome.json")).unwrap())
            .unwrap();
    assert_eq!(raw["session_id"], SECOND);
}

#[test]
fn default_opt_out_starts_a_fresh_worker_even_after_correction() {
    let (_t, repo, run_dir, path) = fixture(2, false);
    let out = run(&repo, &run_dir, &path, false);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state(&run_dir)["state"]["status"], "complete");
    assert!(plan(&run_dir, 2)["session_in"].is_null());
}

#[test]
fn explicit_reset_and_new_controller_process_never_recover_previous_worker_session() {
    let (_t, repo, run_dir, path) = fixture(1, true);
    let _out = run(&repo, &run_dir, &path, false);
    assert_eq!(state(&run_dir)["state"]["status"], "stopped");
    let reset = cli(
        &[
            "reset",
            run_dir.to_str().unwrap(),
            "--acknowledge-external-effects",
        ],
        &path,
        false,
    );
    assert!(
        reset.status.success(),
        "{}",
        String::from_utf8_lossy(&reset.stderr)
    );
    let resumed = cli(&["resume", run_dir.to_str().unwrap()], &path, false);
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert!(plan(&run_dir, 2)["session_in"].is_null());
    assert_eq!(state(&run_dir)["state"]["status"], "complete");
}

#[test]
fn resumed_worker_reporting_different_session_stops_without_review_or_replay() {
    let (_t, repo, run_dir, path) = fixture(2, true);
    let out = run(&repo, &run_dir, &path, true);
    assert!(!out.status.success());
    let record = state(&run_dir);
    assert_eq!(record["state"]["status"], "running");
    assert_eq!(record["state"]["stage"], "first");
    assert_eq!(plan(&run_dir, 2)["session_in"], FIRST);
    assert!(!run_dir.join("actions/action-3").exists());
    assert!(!cli(&["resume", run_dir.to_str().unwrap()], &path, false)
        .status
        .success());
}

#[test]
fn reviewed_self_transition_is_a_new_stage_visit_and_never_resumes_worker() {
    let (_t, repo, run_dir, path) = fixture(2, true);
    let f = repo.join("workflow.toml");
    let t = fs::read_to_string(&f)
        .unwrap()
        .replace(
            "start = \"first\"",
            "start = \"first\"\nmax_stage_transitions = 4",
        )
        .replace(
            "[stages.second]",
            "[stages.first.routes]\nagain = \"first\"\nnext = \"second\"\n[stages.second]",
        );
    fs::write(&f, t).unwrap();
    git(&repo, &["add", "workflow.toml"]);
    git(&repo, &["commit", "-qm", "allow bounded self transition"]);
    let out = run(&repo, &run_dir, &path, false);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state(&run_dir)["state"]["status"], "complete");
    assert!(
        plan(&run_dir, 2)["session_in"].is_null(),
        "a route back to the same stage is a fresh visit"
    );
}

#[test]
fn missing_worker_session_never_falls_back_to_last_or_reviewer_session() {
    let (_t, repo, run_dir, path) = fixture(2, true);
    let out = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run_dir.to_str().unwrap(),
        ])
        .env("PATH", &path)
        .env("FAKE_NO_SESSION", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state(&run_dir)["state"]["status"], "complete");
    assert!(plan(&run_dir, 2)["session_in"].is_null());
    assert!(!plan(&run_dir, 2)["argv"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x == "resume" || x == "--last"));
}

#[test]
fn opted_in_worker_reuses_session_only_after_explicit_unblocker_retry() {
    let (_t, repo, run_dir, path) = fixture(2, true);
    fs::write(repo.join("stop.py"), "import json,os\np=json.load(open(os.environ['SIMON_PACKET'])); print(json.dumps({'action_id':p['action_id'],'outcome':'retry','report':'retry once','candidate_commit':None,'inspected_commit':None}))\n").unwrap();
    git(&repo, &["add", "stop.py"]);
    git(&repo, &["commit", "-qm", "allow one recovery retry"]);
    let out = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run_dir.to_str().unwrap(),
        ])
        .env("PATH", &path)
        .env("FAKE_BLOCK", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state(&run_dir)["state"]["status"], "complete");
    assert!(
        plan(&run_dir, 1)["session_in"].is_null(),
        "Unblocker is independent"
    );
    assert_eq!(plan(&run_dir, 2)["session_in"], FIRST);
    assert_eq!(
        plan(&run_dir, 4)["session_in"],
        FIRST,
        "same-stage review correction remains within worker session"
    );
}
