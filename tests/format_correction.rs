#![cfg(target_os = "macos")]
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

const FAKE: &str = r##"#!/usr/bin/env python3
import json, os, pathlib, sys
p=json.loads(pathlib.Path(os.environ['SIMON_PACKET']).read_text())
prompt=sys.stdin.read()
mode=os.environ['TEST_MODE']
if prompt.startswith('Your completed action'):
    if mode == 'bad': response={'route':'unauthorized'}
    elif mode == 'bad_format': response='still not json'
    elif mode == 'malformed':
        assert 'route at the top level' in prompt and 'not inside report' in prompt
        response={'action_id':p['action_id'],'outcome':'complete','report':'finished','route':'done'}
    else: response={'route':'done'}
else:
    pathlib.Path('result.json').write_text('{"completed":true}\n')
    response={'action_id':p['action_id'],'outcome':'complete','report':'The route is done.'}
    if mode == 'provider_failure': sys.exit(4)
    if mode == 'bad_identity': response.pop('action_id')
    if mode == 'malformed': response=json.dumps({'action_id':p['action_id'],'outcome':'complete','report':{'route':'done'}})[:-1]
    elif mode == 'valid': response['route']='done'
text=response if isinstance(response,str) else json.dumps(response)
print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':text}}))
print(json.dumps({'type':'turn.completed'}))
"##;
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
fn exercise(mode: &str) -> (tempfile::TempDir, std::path::PathBuf, std::process::Output) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let bin = temp.path().join("bin");
    let run = temp.path().join("run");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&bin).unwrap();
    fs::write(bin.join("codex"), FAKE).unwrap();
    fs::set_permissions(bin.join("codex"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(repo.join("instructions.md"), "Do the task.").unwrap();
    fs::write(
        repo.join("workflow.toml"),
        r#"schema_version = 1
name = "format-test"
start = "first"
max_stage_transitions = 2
[roles.worker]
kind = "agent"
adapter = "codex"
model = "test"
profile = "instructions.md"
[stages.first]
worker = "worker"
instructions = "instructions.md"
outputs = ["result.json"]
on_pass = "complete"
[stages.first.routes]
done = "complete"
"#,
    )
    .unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let out = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ])
        .env("PATH", path)
        .env("TEST_MODE", mode)
        .output()
        .unwrap();
    (temp, run, out)
}
fn state(run: &Path) -> Value {
    serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap()
}
#[test]
fn missing_route_is_corrected_once_without_replaying_work() {
    let (_t, run, out) = exercise("missing");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(state(&run)["state"]["status"], "complete");
    assert_eq!(
        state(&run)["state"]["last_transition"]["selected_route"],
        "done"
    );
    assert!(run
        .join("actions/action-0/format-correction/outcome.json")
        .exists());
    assert!(!run.join("actions/action-1").exists());
}
#[test]
fn malformed_response_is_corrected_once() {
    let (_t, run, out) = exercise("malformed");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(run
        .join("actions/action-0/format-correction/outcome.json")
        .exists());
    assert_eq!(state(&run)["state"]["status"], "complete");
}
#[test]
fn correction_cannot_choose_undeclared_route() {
    let (_t, run, out) = exercise("bad");
    assert!(!out.status.success());
    assert_eq!(state(&run)["state"]["status"], "running");
    assert!(String::from_utf8_lossy(&out.stderr).contains("undeclared route"));
    assert!(!run.join("actions/action-1").exists());
}
#[test]
fn valid_response_needs_no_correction() {
    let (_t, run, out) = exercise("valid");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!run.join("actions/action-0/format-correction").exists());
}
#[test]
fn provider_failure_is_not_retried() {
    let (_t, run, out) = exercise("provider_failure");
    assert!(!out.status.success());
    assert!(!run.join("actions/action-0/format-correction").exists());
}

#[test]
fn bad_correction_stops_after_one_attempt() {
    let (_t, run, out) = exercise("bad_format");
    assert!(!out.status.success());
    assert!(run
        .join("actions/action-0/format-correction/outcome.json")
        .exists());
    assert_eq!(state(&run)["state"]["status"], "running");
    assert!(!run.join("actions/action-1").exists());
}
#[test]
fn invalid_identity_is_not_a_format_retry() {
    let (_t, run, out) = exercise("bad_identity");
    assert!(!out.status.success());
    assert!(!run.join("actions/action-0/format-correction").exists());
}
