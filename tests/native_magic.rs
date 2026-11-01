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
/// Explicit opt-in: launches two local native simulator games and needs real assets.
#[test]
#[ignore = "requires an explicitly prepared isolated native Magic runtime and executes two games"]
fn real_native_diagnostic_runs_through_controller() {
    let runtime = std::env::var("SIMON_MAGIC_RUNTIME_ROOT")
        .expect("set SIMON_MAGIC_RUNTIME_ROOT to the isolated runtime copy");
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("project");
    fs::create_dir(&repo).unwrap();
    for item in fs::read_dir("examples/magic-native").unwrap() {
        let item = item.unwrap();
        fs::copy(item.path(), repo.join(item.file_name())).unwrap();
    }
    for (deck, checkpoint) in [("angels", 2), ("miirym", 2), ("azula", 1), ("aang", 1)] {
        let rel = format!("decks/{deck}/checkpoint_{checkpoint}");
        fs::create_dir_all(repo.join(&rel)).unwrap();
        for name in ["deck_list.csv", "GUIDE.md"] {
            fs::copy(
                Path::new(&runtime).join("repo").join(&rel).join(name),
                repo.join(&rel).join(name),
            )
            .unwrap();
        }
    }
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = temp.path().join("run");
    let result = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .arg("run")
        .arg(repo.join("workflow.toml"))
        .arg("--project")
        .arg(&repo)
        .arg("--run-dir")
        .arg(&run)
        .env("SIMON_MAGIC_RUNTIME_ROOT", &runtime)
        .output()
        .unwrap();
    if !result.status.success() {
        let errors = (0..8)
            .map(|i| {
                format!(
                    "action-{i}: {}",
                    fs::read_to_string(run.join(format!("actions/action-{i}/stderr.txt")))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let preserved = temp.keep();
        panic!(
            "stdout={} stderr={}\n{}\npreserved={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr),
            errors,
            preserved.display()
        );
    }
    let state: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(state["state"]["status"], "complete");
    let expected = [
        ("worker", "blocked"),
        ("unblocker", "retry"),
        ("worker", "complete"),
        ("reviewer", "changes_required"),
        ("worker", "complete"),
        ("reviewer", "pass"),
        ("worker", "complete"),
        ("reviewer", "pass"),
    ];
    for (i, (gate, outcome)) in expected.iter().enumerate() {
        let dir = run.join(format!("actions/action-{i}"));
        let packet: Value =
            serde_json::from_slice(&fs::read(dir.join("packet.json")).unwrap()).unwrap();
        let transport: Value =
            serde_json::from_slice(&fs::read(dir.join("outcome.json")).unwrap()).unwrap();
        let response: Value =
            serde_json::from_str(transport["response"].as_str().unwrap()).unwrap();
        assert_eq!(packet["gate"], *gate);
        assert_eq!(response["outcome"], *outcome);
    }
    if std::env::var_os("SIMON_MAGIC_KEEP_EVIDENCE").is_some() {
        println!("native-magic-evidence={}", temp.keep().display());
    }
}

/// The isolated fixture replaces only the two ps invocations it actually uses.
#[cfg(target_os = "macos")]
#[test]
fn fixture_process_identity_matches_system_ps() {
    let pid = std::process::id().to_string();
    let args = ["-p", pid.as_str(), "-o", "lstart="];
    let expected = Command::new("/bin/ps").args(args).output().unwrap();
    assert!(expected.status.success());
    let actual = Command::new("python3")
        .arg("examples/magic-native/ps_compat.py")
        .args(args)
        .output()
        .unwrap();
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout).trim(),
        String::from_utf8_lossy(&expected.stdout).trim()
    );
    let nonexistent = Command::new("python3")
        .arg("examples/magic-native/ps_compat.py")
        .args(["-p", "2147483647", "-o", "lstart="])
        .output()
        .unwrap();
    assert!(!nonexistent.status.success());
}

#[test]
fn fixture_host_observation_never_fabricates_java_metrics() {
    let java = Command::new("/usr/bin/pgrep")
        .args(["-x", "java"])
        .status()
        .unwrap();
    let actual = Command::new("python3")
        .args([
            "examples/magic-native/ps_compat.py",
            "-axo",
            "pid,pcpu,rss,comm",
        ])
        .output()
        .unwrap();
    if java.code() == Some(1) {
        assert!(
            actual.status.success(),
            "{}",
            String::from_utf8_lossy(&actual.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&actual.stdout).lines().count(), 1);
    } else {
        assert!(!actual.status.success());
    }
}
