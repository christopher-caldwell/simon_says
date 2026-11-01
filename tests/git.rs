#[path = "../src/git.rs"]
mod git;
use git::Worktree;
use std::{fs, path::Path, process::Command};
fn command(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_owned()
}
fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let repo = d.path().join("repo");
    let run = d.path().join("run");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&run).unwrap();
    command(&repo, &["init", "-q"]);
    command(&repo, &["config", "user.email", "test@example.invalid"]);
    command(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("data"), "baseline").unwrap();
    command(&repo, &["add", "data"]);
    command(&repo, &["commit", "-qm", "baseline"]);
    (d, repo, run)
}
#[test]
fn clean_isolated_worktree_and_exact_candidates() {
    let (_d, repo, run) = fixture();
    let w = Worktree::create(&repo, &run).unwrap();
    assert_eq!(w.head().unwrap(), w.baseline);
    w.verify_clean().unwrap();
    fs::write(w.path.join("data"), "candidate").unwrap();
    assert!(w.verify_clean().is_err());
    command(&w.path, &["add", "data"]);
    command(&w.path, &["commit", "-qm", "candidate"]);
    let c = w.head().unwrap();
    w.verify_candidate(&c, &w.baseline).unwrap();
    assert!(w.verify_candidate(&w.baseline, &w.baseline).is_err());
    assert!(w.verify_candidate("HEAD", &w.baseline).is_err());
    assert_eq!(fs::read_to_string(repo.join("data")).unwrap(), "baseline");
    let review_dir = run.join("review-1");
    fs::create_dir(&review_dir).unwrap();
    let review = w.create_review(&c, &review_dir).unwrap();
    assert_eq!(review.head().unwrap(), c);
    fs::write(review.path.join("data"), "tamper").unwrap();
    assert!(review.verify_clean().is_err());
    assert_eq!(
        fs::read_to_string(w.path.join("data")).unwrap(),
        "candidate"
    );
}
#[test]
fn source_dirty_and_existing_worktree_are_refused() {
    let (_d, repo, run) = fixture();
    fs::write(repo.join("untracked"), "x").unwrap();
    assert!(Worktree::create(&repo, &run).is_err());
    fs::remove_file(repo.join("untracked")).unwrap();
    fs::create_dir(run.join("worktree")).unwrap();
    assert!(Worktree::create(&repo, &run).is_err());
}
#[test]
fn reset_preserves_untracked_and_ignored_and_checks_ownership() {
    let (_d, repo, run) = fixture();
    let w = Worktree::create(&repo, &run).unwrap();
    fs::write(w.path.join("data"), "tracked change").unwrap();
    fs::write(w.path.join("other"), "valuable").unwrap();
    assert!(w.reset_to(&w.baseline).is_err());
    assert_eq!(
        fs::read_to_string(w.path.join("other")).unwrap(),
        "valuable"
    );
    fs::remove_file(w.path.join("other")).unwrap();
    // An ignored path must not be overwritten or removed either.
    command(&w.path, &["config", "core.excludesFile", "/dev/null"]);
    fs::write(w.path.join(".gitignore"), "ignored\n").unwrap();
    command(&w.path, &["add", ".gitignore"]);
    command(&w.path, &["commit", "-qm", "ignore"]);
    fs::write(w.path.join("ignored"), "valuable").unwrap();
    assert!(w.reset_to(&w.baseline).is_err());
    assert_eq!(
        fs::read_to_string(w.path.join("ignored")).unwrap(),
        "valuable"
    );
    fs::remove_file(w.path.join("ignored")).unwrap();
    w.reset_to(&w.baseline).unwrap();
    assert_eq!(fs::read_to_string(w.path.join("data")).unwrap(), "baseline");
    let mut forged = w.clone();
    forged.git_dir = repo.join(".git");
    assert!(forged.reset_to(&w.baseline).is_err());
}
#[test]
fn non_descendant_candidate_is_rejected() {
    let (_d, repo, run) = fixture();
    let w = Worktree::create(&repo, &run).unwrap();
    command(&w.path, &["checkout", "--orphan", "unrelated"]);
    command(&w.path, &["commit", "--allow-empty", "-qm", "unrelated"]);
    let c = w.head().unwrap();
    assert!(w.verify_candidate(&c, &w.baseline).is_err());
}
