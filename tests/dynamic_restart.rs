#![cfg(target_os = "macos")]

use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn interrupted_dynamically_dispatched_repair_is_not_replayed() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let run = temp.path().join("run");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/dynamic-repair").unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
    }
    fs::write(repo.join("mode.json"), "{\"mode\":\"repairable\"}\n").unwrap();
    fs::write(repo.join("engine.json"), "{\"correct\":false}\n").unwrap();
    let path = repo.join("action.py");
    let code = fs::read_to_string(&path).unwrap().replace(
        "observation = read(\"simulation.json\")\n        if not observation[\"issue\"]:",
        "observation = read(\"simulation.json\")\n        Path(os.environ[\"SIMON_SCRATCH\"]).joinpath(\"started\").write_text(\"started\")\n        import time\n        time.sleep(3)\n        return\n        if not observation[\"issue\"]:",
    );
    assert!(code.contains("time.sleep(3)"));
    fs::write(path, code).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let mut controller = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let marker = run.join("actions/action-2/scratch/started");
    let limit = Instant::now() + Duration::from_secs(30);
    while !marker.exists() && Instant::now() < limit {
        assert!(
            controller.try_wait().unwrap().is_none(),
            "controller stopped before repair began"
        );
        thread::sleep(Duration::from_millis(20));
    }
    if !marker.exists() {
        let _ = controller.kill();
        let _ = controller.wait();
        panic!("dynamically routed repair did not start");
    }
    let before = json(&run.join("state.json"));
    assert_eq!(before["state"]["status"], "running");
    assert_eq!(before["state"]["stage"], "repair");
    assert_eq!(
        before["state"]["last_transition"]["selected_route"],
        "issues"
    );
    let checkpoint = before["state"]["accepted_commit"]
        .as_str()
        .unwrap()
        .to_owned();
    controller.kill().unwrap();
    controller.wait().unwrap();
    let resumed = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(["resume", run.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !resumed.status.success(),
        "uncertain repair must not be replayed"
    );
    let after = json(&run.join("state.json"));
    assert_eq!(after["state"]["status"], "running");
    assert_eq!(after["state"]["accepted_commit"], checkpoint);
    assert!(!run.join("actions/action-3").exists());
    // The fixture child performs no action after its bounded sleep; wait for it
    // to exit before the temporary directory is removed. No external effects.
    thread::sleep(Duration::from_secs(4));
}
