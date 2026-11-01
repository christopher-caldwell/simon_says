#![cfg(target_os = "macos")]

use serde_json::Value;
use std::{
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
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
fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let run = temp.path().join("run");
    let trigger = run.join("actions/action-0/scratch/trigger");
    let effect = temp.path().join("filter-effect");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/data-report").unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
        }
    }
    let script = temp.path().join("hanging-filter.sh");
    fs::write(&script, "#!/bin/sh\nif [ -f \"$SIMON_GIT_TEST_TRIGGER\" ]; then\n  echo invoked >> \"$SIMON_GIT_TEST_EFFECT\"\n  sleep 80\n  exit 1\nfi\ncat\n").unwrap();
    fs::write(repo.join(".gitattributes"), "input.json filter=slow\n").unwrap();
    let action = repo.join("workflow_action.py");
    let body = fs::read_to_string(&action).unwrap();
    let needle = "        output(packet, \"complete\", f\"Created {path} from the declared input.\", candidate=git(\"rev-parse\", \"HEAD\"))";
    assert_eq!(body.matches(needle).count(), 1);
    fs::write(&action, body.replace(needle, &format!("        Path(os.environ[\"SIMON_SCRATCH\"]).joinpath(\"trigger\").write_text(\"started\")\n{needle}"))).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    let quoted = script.to_string_lossy().replace('\'', "'\\''");
    git(
        &repo,
        &[
            "config",
            "filter.slow.clean",
            &format!("/bin/sh '{quoted}'"),
        ],
    );
    git(&repo, &["config", "filter.slow.required", "true"]);
    git(&repo, &["config", "filter.slow.smudge", "cat"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    (temp, repo, run, trigger, effect)
}

fn execute_with_watchdog(
    repo: &Path,
    run: &Path,
    trigger: &Path,
    effect: &Path,
) -> (Output, Duration) {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            repo.join("workflow.toml").to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ])
        .env("SIMON_GIT_TEST_TRIGGER", trigger)
        .env("SIMON_GIT_TEST_EFFECT", effect)
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let limit = start + Duration::from_secs(90);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= limit {
            // Test watchdog handles a broken implementation without killing
            // any process outside the test controller's own group.
            unsafe {
                libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = child.wait();
            panic!("controller exceeded the 90-second test watchdog");
        }
        thread::sleep(Duration::from_millis(100));
    }
    let elapsed = start.elapsed();
    (child.wait_with_output().unwrap(), elapsed)
}

#[test]
fn hanging_preflight_filter_times_out_before_run_creation() {
    let (_temp, repo, run, _trigger, effect) = fixture();
    let trigger = repo.parent().unwrap().join("preflight-trigger");
    fs::write(&trigger, "triggered").unwrap();
    let (output, elapsed) = execute_with_watchdog(&repo, &run, &trigger, &effect);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("timed out")
            && error.contains("before creating a run record")
            && error.contains("external effects"),
        "{error}"
    );
    assert!(
        elapsed >= Duration::from_secs(55) && elapsed < Duration::from_secs(90),
        "{elapsed:?}"
    );
    assert!(
        !run.exists(),
        "preflight must not leave a partially claimed run"
    );
    assert!(
        effect.exists(),
        "side effects before timeout must not be hidden"
    );
    assert!(repo.join("input.json").exists());
}

#[test]
fn hanging_filter_after_action_leaves_running_state_and_refuses_replay() {
    let (_temp, repo, run, trigger, effect) = fixture();
    let (output, elapsed) = execute_with_watchdog(&repo, &run, &trigger, &effect);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("timed out") && error.contains("external effects"),
        "{error}"
    );
    assert!(
        elapsed >= Duration::from_secs(55) && elapsed < Duration::from_secs(90),
        "{elapsed:?}"
    );
    assert!(effect.exists(), "Git filter side effects remain visible");
    let record: Value = serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap();
    assert_eq!(record["state"]["status"], "running");
    assert_eq!(record["state"]["gate"], "worker");
    assert!(run.join("actions/action-0/outcome.json").is_file());
    assert!(!run.join("actions/action-1").exists());
    let resume = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(["resume", run.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !resume.status.success(),
        "uncertain action must not be replayed"
    );
    assert!(!run.join("actions/action-1").exists());
    assert!(repo.join("input.json").exists());
}

#[test]
fn hanging_checkout_filter_leaves_initializing_intent_and_refuses_replay() {
    let (_temp, repo, run, _trigger, effect) = fixture();
    let trigger = repo.parent().unwrap().join("checkout-trigger");
    fs::write(&trigger, "triggered").unwrap();
    // Clean/hash checks must remain fast; only the new checkout's smudge
    // filter stalls, after its Creating intent was durably persisted.
    let script = repo.parent().unwrap().join("hanging-filter.sh");
    let quoted = script.to_string_lossy().replace('\'', "'\\''");
    git(&repo, &["config", "filter.slow.clean", "cat"]);
    git(
        &repo,
        &[
            "config",
            "filter.slow.smudge",
            &format!("/bin/sh '{quoted}'"),
        ],
    );
    let (output, elapsed) = execute_with_watchdog(&repo, &run, &trigger, &effect);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("timed out") && error.contains("external effects"),
        "{error}"
    );
    assert!(
        elapsed >= Duration::from_secs(55) && elapsed < Duration::from_secs(90),
        "{elapsed:?}"
    );
    assert!(
        effect.exists(),
        "Git checkout filter side effects remain visible"
    );
    let record: Value = serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap();
    assert_eq!(record["state"]["status"], "initializing");
    assert_eq!(record["worktree"]["phase"], "creating");
    let resume = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(["resume", run.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !resume.status.success(),
        "uncertain checkout must not be replayed"
    );
    let recover = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(["recover-init", run.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        !recover.status.success(),
        "uncertain checkout requires explicit reconciliation"
    );
    let after: Value = serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap();
    assert_eq!(after["state"]["status"], "initializing");
    assert_eq!(after["worktree"]["phase"], "creating");
    assert!(repo.join("input.json").exists());
}
