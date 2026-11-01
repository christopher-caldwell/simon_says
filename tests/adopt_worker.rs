#![cfg(target_os = "macos")]
use serde_json::Value;
use std::{
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

fn git(repo: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf, String) {
    fixture_with_options(false, false)
}
fn fixture_with_invalid_output(invalid: bool) -> (tempfile::TempDir, PathBuf, PathBuf, String) {
    fixture_with_options(invalid, false)
}
fn fixture_with_options(
    invalid: bool,
    filtered: bool,
) -> (tempfile::TempDir, PathBuf, PathBuf, String) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/data-report").unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
        }
    }
    fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    if filtered {
        fs::write(repo.join(".gitattributes"), "input.json filter=slow\n").unwrap();
    }
    let script = repo.join("workflow_action.py");
    let mut text = fs::read_to_string(&script).unwrap();
    if invalid {
        text = text.replace(
            "Path(path).write_text(json.dumps(expected, indent=2) + \"\\n\")",
            "Path(path).write_text('{}\\n')",
        );
    }
    let needle = "        output(packet, \"complete\", f\"Created {path} from the declared input.\", candidate=git(\"rev-parse\", \"HEAD\"))";
    assert_eq!(text.matches(needle).count(), 1);
    fs::write(script, text.replace(needle, &format!("        if stage == 'validate':\n            Path('target').mkdir(exist_ok=True)\n            Path('target/evidence').write_text('preserve')\n{needle}"))).unwrap();
    git(&repo, &["init", "-q"]);
    if filtered {
        git(&repo, &["config", "filter.slow.clean", "cat"]);
        git(&repo, &["config", "filter.slow.smudge", "cat"]);
        git(&repo, &["config", "filter.slow.required", "true"]);
    }
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = tmp.path().join("run");
    let output = cli(&[
        "run",
        repo.join("workflow.toml").to_str().unwrap(),
        "--project",
        repo.to_str().unwrap(),
        "--run-dir",
        run.to_str().unwrap(),
    ]);
    assert!(
        !output.status.success(),
        "fixture must stop at ignored output"
    );
    assert_eq!(state(&run)["state"]["status"], "running");
    let candidate = git(&run.join("worktree"), &["rev-parse", "HEAD"]);
    (tmp, repo, run, candidate)
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args(args)
        .output()
        .unwrap()
}
fn state(run: &Path) -> Value {
    serde_json::from_slice(&fs::read(run.join("state.json")).unwrap()).unwrap()
}
fn adopt(run: &Path, action: &str, candidate: &str, ack: bool) -> Output {
    adopt_with_reconcile(run, action, candidate, ack, false)
}
fn adopt_with_reconcile(
    run: &Path,
    action: &str,
    candidate: &str,
    ack: bool,
    reconcile: bool,
) -> Output {
    let mut args = vec![
        "adopt-worker",
        run.to_str().unwrap(),
        "--expected-action",
        action,
        "--expected-candidate",
        candidate,
    ];
    if ack {
        args.push("--acknowledge-external-effects");
    }
    if reconcile {
        args.push("--reconcile-check");
    }
    cli(&args)
}
fn preserve(run: &Path) {
    fs::rename(
        run.join("worktree/target"),
        run.join("actions/action-0/scratch/preserved-target"),
    )
    .unwrap();
}
#[test]
fn adopts_verified_candidate_without_replaying_worker_and_survives_restart() {
    let (_tmp, _repo, run, candidate) = fixture();
    assert!(!adopt(&run, "action-0", &candidate, false).status.success());
    assert!(
        !adopt(&run, "action-0", &candidate, true).status.success(),
        "ignored content must be preserved and reconciled first"
    );
    assert_eq!(state(&run)["state"]["status"], "adoption_checking");
    assert!(!cli(&["resume", run.to_str().unwrap()]).status.success());
    assert!(!cli(&[
        "reset",
        run.to_str().unwrap(),
        "--acknowledge-external-effects"
    ])
    .status
    .success());
    preserve(&run);
    assert!(!adopt(&run, "action-1", &candidate, true).status.success());
    assert!(
        !adopt(&run, "action-0", &candidate, true).status.success(),
        "checking state must require explicit reconciliation"
    );
    let result = adopt_with_reconcile(&run, "action-0", &candidate, true, true);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(state(&run)["state"]["gate"], "reviewer");
    assert_eq!(state(&run)["state"]["candidate_commit"], candidate);
    assert!(
        !adopt(&run, "action-0", &candidate, true).status.success(),
        "adoption must not repeat after restart"
    );
    let resumed = cli(&["resume", run.to_str().unwrap()]);
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert_eq!(state(&run)["state"]["status"], "complete");
    assert_eq!(
        fs::read_to_string(run.join("actions/action-0/scratch/preserved-target/evidence")).unwrap(),
        "preserve"
    );
    let packet: Value =
        serde_json::from_slice(&fs::read(run.join("actions/action-1/packet.json")).unwrap())
            .unwrap();
    assert_eq!(packet["gate"], "reviewer");
    assert_eq!(packet["candidate_commit"], candidate);
}
#[test]
fn missing_or_forged_outcome_never_advances() {
    for mutation in ["missing", "forged"] {
        let (_tmp, _repo, run, candidate) = fixture();
        preserve(&run);
        let path = run.join("actions/action-0/outcome.json");
        if mutation == "missing" {
            fs::remove_file(path).unwrap();
        } else {
            let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            value["response"] = Value::String("{}".into());
            fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        }
        assert!(!adopt(&run, "action-0", &candidate, true).status.success());
        assert!(matches!(
            state(&run)["state"]["status"].as_str(),
            Some("running" | "adoption_checking")
        ));
        assert!(!run.join("actions/action-1").exists());
    }
}
#[test]
fn legacy_wrapped_worker_record_is_explicitly_refused_without_advancing() {
    let (_tmp, _repo, run, candidate) = fixture();
    preserve(&run);
    let path = run.join("actions/action-0/invocation.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["program"] = Value::String("/usr/bin/sandbox-exec".into());
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    let result = adopt(&run, "action-0", &candidate, true);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("legacy sandbox-wrapped Worker records")
    );
    assert_eq!(state(&run)["state"]["status"], "running");
    assert!(!run.join("actions/action-1").exists());
}
#[test]
fn forged_packet_or_changed_checkout_never_advances() {
    for mutation in ["packet", "invocation", "checkout"] {
        let (_tmp, _repo, run, candidate) = fixture();
        preserve(&run);
        if mutation == "packet" {
            let path = run.join("actions/action-0/packet.json");
            let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            value["role"] = Value::String("forged".into());
            fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        } else if mutation == "invocation" {
            let path = run.join("actions/action-0/invocation.json");
            let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            value["argv"][0] = Value::String("forged".into());
            fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        } else {
            fs::write(run.join("worktree/validation.json"), "{}\n").unwrap();
        }
        assert!(!adopt(&run, "action-0", &candidate, true).status.success());
        assert!(matches!(
            state(&run)["state"]["status"].as_str(),
            Some("running" | "adoption_checking")
        ));
        assert!(!run.join("actions/action-1").exists());
    }
}
#[test]
fn schema_invalid_candidate_is_not_adopted() {
    let (_tmp, _repo, run, candidate) = fixture_with_invalid_output(true);
    preserve(&run);
    assert!(!adopt(&run, "action-0", &candidate, true).status.success());
    assert_eq!(state(&run)["state"]["status"], "adoption_checking");
    assert!(!run.join("actions/action-1").exists());
}
#[test]
fn resumed_worker_without_anchored_session_provenance_is_refused() {
    let (_tmp, _repo, run, candidate) = fixture();
    preserve(&run);
    let path = run.join("actions/action-0/invocation.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["session_in"] = Value::String("11111111-1111-4111-8111-111111111111".into());
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(!adopt(&run, "action-0", &candidate, true).status.success());
    assert_eq!(state(&run)["state"]["status"], "running");
}
#[test]
fn live_recorded_process_group_prevents_adoption() {
    let (_tmp, _repo, run, candidate) = fixture();
    preserve(&run);
    let mut child = Command::new("sh")
        .args(["-c", "sleep 15"])
        .process_group(0)
        .spawn()
        .unwrap();
    let pid = child.id();
    fs::write(
        run.join("actions/action-0/process.json"),
        serde_json::to_vec(&serde_json::json!({"pid":pid,"process_group":pid})).unwrap(),
    )
    .unwrap();
    let output = adopt(&run, "action-0", &candidate, true);
    // The child is unreaped, so this test-owned process group cannot be reused.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
    child.wait().unwrap();
    assert!(!output.status.success());
    assert_eq!(state(&run)["state"]["status"], "adoption_checking");
    assert!(!run.join("actions/action-1").exists());
}
#[test]
fn legacy_packet_without_empty_scratch_environment_is_supported() {
    let (_tmp, _repo, run, candidate) = fixture();
    preserve(&run);
    let path = run.join("actions/action-0/packet.json");
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(value
        .as_object_mut()
        .unwrap()
        .remove("scratch_env")
        .is_some());
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
    let result = adopt(&run, "action-0", &candidate, true);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(state(&run)["state"]["gate"], "reviewer");
}
#[test]
fn interrupted_adoption_check_requires_explicit_reconciliation() {
    let (tmp, repo, run, candidate) = fixture_with_options(false, true);
    preserve(&run);
    let marker = tmp.path().join("filter-marker");
    let script = tmp.path().join("slow-filter.sh");
    fs::write(
        &script,
        "#!/bin/sh\nprintf invoked >> \"$SIMON_ADOPT_TEST_MARKER\"\nsleep 3\ncat\n",
    )
    .unwrap();
    let quoted = script.to_string_lossy().replace('\'', "'\\''");
    git(
        &repo,
        &[
            "config",
            "filter.slow.clean",
            &format!("/bin/sh '{quoted}'"),
        ],
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "adopt-worker",
            run.to_str().unwrap(),
            "--expected-action",
            "action-0",
            "--expected-candidate",
            &candidate,
            "--acknowledge-external-effects",
        ])
        .env("SIMON_ADOPT_TEST_MARKER", &marker)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() && Instant::now() < deadline && child.try_wait().unwrap().is_none() {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !marker.exists() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("adoption did not enter the test Git filter");
    }
    assert_eq!(state(&run)["state"]["status"], "adoption_checking");
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(!cli(&["resume", run.to_str().unwrap()]).status.success());
    assert!(!cli(&[
        "reset",
        run.to_str().unwrap(),
        "--acknowledge-external-effects"
    ])
    .status
    .success());
    assert!(!adopt(&run, "action-0", &candidate, true).status.success());
    // The orphaned fixture filter has a finite lifetime; wait for it to exit
    // before explicitly reconciling and changing the fixture's Git config.
    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(fs::read_to_string(marker).unwrap(), "invoked");
    git(&repo, &["config", "filter.slow.clean", "cat"]);
    let output = adopt_with_reconcile(&run, "action-0", &candidate, true, true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(state(&run)["state"]["gate"], "reviewer");
}
