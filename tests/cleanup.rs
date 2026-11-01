#![cfg(target_os = "macos")]

pub use simon_says::{git, provider, routing, store};
#[path = "../src/cleanup.rs"]
mod cleanup;
use cleanup::cleanup_reviews;
use simon_says::{controller, routing::Status};
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
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
    String::from_utf8(output.stdout).unwrap()
}
fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("project");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/data-report").unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
        }
    }
    fs::write(repo.join(".gitignore"), "ignored.tmp\n").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let run = dir.path().join("run");
    controller::initialize(&repo.join("workflow.toml"), &repo, &run).unwrap();
    (dir, repo, run)
}
fn complete(repo: &Path, run: &Path) -> String {
    let state = controller::execute(run).unwrap();
    assert_eq!(state.state.status, Status::Complete);
    let accepted = state.state.accepted_commit;
    assert!(git(repo, &["cat-file", "-e", &format!("{accepted}^{{commit}}")]).is_empty());
    accepted
}
#[test]
fn completed_review_worktrees_are_removed_without_touching_source_or_evidence() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, repo, run) = setup();
    let accepted = complete(&repo, &run);
    let before = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(cleanup_reviews(&run).unwrap(), 2);
    assert!(!run.join("reviews/action-1/worktree").exists());
    assert!(!run.join("reviews/action-3/worktree").exists());
    assert!(run.join("worktree").exists());
    assert!(run.join("actions/action-1/packet.json").exists());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), before);
    assert!(!repo.join("validation.json").exists());
    assert_eq!(
        controller::status(&run).unwrap().state.accepted_commit,
        accepted
    );
    assert!(git(
        &repo,
        &["cat-file", "-e", &format!("{accepted}^{{commit}}")]
    )
    .is_empty());
    assert_eq!(cleanup_reviews(&run).unwrap(), 0);
}
#[test]
fn refuses_dirty_or_ignored_review_without_removing_any_worktree() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, repo, run) = setup();
    complete(&repo, &run);
    fs::write(run.join("reviews/action-3/worktree/ignored.tmp"), "keep").unwrap();
    assert!(cleanup_reviews(&run).is_err());
    assert!(run.join("reviews/action-1/worktree").exists());
    assert!(run.join("reviews/action-3/worktree/ignored.tmp").exists());
    fs::remove_file(run.join("reviews/action-3/worktree/ignored.tmp")).unwrap();
    fs::write(run.join("reviews/action-1/worktree/input.json"), "changed").unwrap();
    assert!(cleanup_reviews(&run).is_err());
    assert!(run.join("reviews/action-1/worktree/input.json").exists());
}
#[test]
fn refuses_cleanup_for_incomplete_run() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, _repo, run) = setup();
    assert!(cleanup_reviews(&run).is_err());
    assert!(run.join("worktree").exists());
}
#[cfg(unix)]
#[test]
fn refuses_tampered_action_record_and_symbolic_review_path() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, repo, run) = setup();
    complete(&repo, &run);
    let plan = run.join("actions/action-1/invocation.json");
    let original = fs::read(&plan).unwrap();
    fs::remove_file(&plan).unwrap();
    fs::write(run.join("other-plan"), original).unwrap();
    std::os::unix::fs::symlink(run.join("other-plan"), &plan).unwrap();
    assert!(cleanup_reviews(&run).is_err());
    assert!(run.join("reviews/action-1/worktree").exists());
}
#[test]
fn replacement_review_worktree_with_same_path_and_commit_is_not_removed() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, repo, run) = setup();
    complete(&repo, &run);
    let review = run.join("reviews/action-1/worktree");
    let candidate = git(&review, &["rev-parse", "HEAD"]);
    let path = review.to_str().unwrap();
    git(&repo, &["worktree", "remove", "--", path]);
    git(
        &repo,
        &["worktree", "add", "--detach", "--", path, candidate.trim()],
    );
    assert!(cleanup_reviews(&run).is_err());
    assert!(review.exists());
    assert!(run.join("reviews/action-3/worktree").exists());
}
#[test]
fn legacy_review_packet_without_identity_is_not_cleaned() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, repo, run) = setup();
    complete(&repo, &run);
    let packet = run.join("actions/action-1/packet.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&packet).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("review_worktree");
    fs::write(packet, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(cleanup_reviews(&run).is_err());
    assert!(run.join("reviews/action-1/worktree").exists());
}
#[test]
fn reset_refuses_replacement_worker_and_preserves_tracked_edits() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (_tmp, repo, run) = setup();
    let worktree = run.join("worktree");
    let baseline = git(&worktree, &["rev-parse", "HEAD"]);
    // Force a durable stopped state without touching the checkout.
    let store = simon_says::store::RunStore::open(&run).unwrap();
    let (mut record, _) = store.load().unwrap();
    record.state.status = Status::Stopped;
    record.state.stop_reason = Some("fixture".into());
    store.save(&record).unwrap();
    drop(store);
    let path = worktree.to_str().unwrap();
    git(&repo, &["worktree", "remove", "--", path]);
    git(
        &repo,
        &["worktree", "add", "--detach", "--", path, baseline.trim()],
    );
    fs::write(worktree.join("input.json"), "preserve user edit").unwrap();
    assert!(controller::reset(&run, true).is_err());
    assert_eq!(
        fs::read_to_string(worktree.join("input.json")).unwrap(),
        "preserve user edit"
    );
}
