#![cfg(target_os = "macos")]
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
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
#[test]
fn status_reads_running_action_without_disrupting_exclusive_controller_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    fs::create_dir(&project).unwrap();
    for entry in fs::read_dir("examples/data-report").unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            fs::copy(entry.path(), project.join(entry.file_name())).unwrap();
        }
    }
    fs::write(project.join("workflow_action.py"), "import json,os,time\nfrom pathlib import Path\np=json.loads(Path(os.environ['SIMON_PACKET']).read_text())\n(Path(os.environ['SIMON_SCRATCH'])/'started').write_text('yes')\ntime.sleep(2)\nprint(json.dumps({'action_id':p['action_id'],'outcome':'blocked','report':'Controlled test blocker.','candidate_commit':None,'inspected_commit':p.get('candidate_commit') if p['gate']=='reviewer' else None}))\n").unwrap();
    git(&project, &["init", "-q"]);
    git(&project, &["config", "user.name", "Status Fixture"]);
    git(
        &project,
        &["config", "user.email", "fixture@example.invalid"],
    );
    git(&project, &["add", "."]);
    git(&project, &["commit", "-qm", "baseline"]);
    let run = tmp.path().join("run");
    let binary = env!("CARGO_BIN_EXE_simon-says");
    let mut child = Command::new(binary)
        .args([
            "run",
            project.join("workflow.toml").to_str().unwrap(),
            "--project",
            project.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let marker = run.join("actions/action-0/scratch/started");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !marker.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if !marker.exists() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("worker did not start");
    }
    let status = Command::new(binary)
        .args(["status", run.to_str().unwrap()])
        .output()
        .unwrap();
    let was_running = child.try_wait().unwrap().is_none();
    let finish = child.wait().unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let observed: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert!(was_running);
    assert_eq!(observed["state"]["status"], "running");
    assert_eq!(observed["state"]["action_id"], "action-0");
    assert!(!finish.success(), "fixture must stop at blocked Unblocker");
}
