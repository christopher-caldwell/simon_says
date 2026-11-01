use simon_says::git::Worktree;
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
#[test]
fn hidden_index_flags_cannot_disguise_dirty_worker() {
    let (_tmp, repo, run) = fixture();
    let w = Worktree::create(&repo, &run).unwrap();
    command(&w.path, &["update-index", "--assume-unchanged", "data"]);
    fs::write(w.path.join("data"), "hidden change").unwrap();
    assert!(w.verify_clean().is_err());
    assert!(w.reset_to(&w.baseline).is_err());
}
#[test]
fn detects_tracked_content_changed_with_cached_stat_metadata() {
    use std::time::Duration;
    let (_tmp, repo, run) = fixture();
    command(&repo, &["config", "core.trustctime", "false"]);
    let w = Worktree::create(&repo, &run).unwrap();
    std::thread::sleep(Duration::from_millis(1200));
    command(&w.path, &["update-index", "--refresh"]);
    let path = w.path.join("data");
    let original = fs::metadata(&path).unwrap().modified().unwrap();
    fs::write(&path, "changed!").unwrap(); // same length as "baseline"
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(original))
        .unwrap();
    assert_eq!(
        command(&w.path, &["status", "--porcelain"]),
        "",
        "fixture must exercise Git stat-cache evasion"
    );
    assert!(w.verify_candidate(&w.baseline, &w.baseline).is_err());
}
#[test]
fn verifies_filtered_checkout_contents_using_git_clean_filter() {
    let (_tmp, repo, run) = fixture();
    command(&repo, &["config", "filter.test.clean", "tr A-Z a-z"]);
    command(&repo, &["config", "filter.test.smudge", "tr a-z A-Z"]);
    fs::write(repo.join(".gitattributes"), "filtered filter=test\n").unwrap();
    fs::write(repo.join("filtered"), "lowercase\n").unwrap();
    command(&repo, &["add", ".gitattributes", "filtered"]);
    command(&repo, &["commit", "-qm", "filtered file"]);
    let w = Worktree::create(&repo, &run).unwrap();
    assert_eq!(
        fs::read_to_string(w.path.join("filtered")).unwrap(),
        "LOWERCASE\n"
    );
    w.verify_candidate(&w.baseline, &w.baseline).unwrap();
}
#[cfg(unix)]
#[test]
fn validates_tracked_symlink_and_executable_mode() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let (_tmp, repo, run) = fixture();
    symlink("data", repo.join("link")).unwrap();
    fs::write(repo.join("script"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(repo.join("script"), fs::Permissions::from_mode(0o755)).unwrap();
    command(&repo, &["add", "link", "script"]);
    command(&repo, &["commit", "-qm", "modes"]);
    let w = Worktree::create(&repo, &run).unwrap();
    w.verify_candidate(&w.baseline, &w.baseline).unwrap();
    fs::set_permissions(w.path.join("script"), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(w.verify_clean().is_err());
}
#[test]
fn hashes_batched_and_quoted_paths_without_confusion() {
    let (_tmp, repo, run) = fixture();
    for name in [
        "a space",
        "tab\tname",
        "-dash",
        "newline\nname",
        "quote\"name",
        "back\\slash",
    ] {
        fs::write(repo.join(name), format!("contents: {name}")).unwrap();
    }
    command(&repo, &["add", "--all"]);
    command(&repo, &["commit", "-qm", "odd filenames"]);
    let w = Worktree::create(&repo, &run).unwrap();
    w.verify_candidate(&w.baseline, &w.baseline).unwrap();
}
#[test]
fn verifies_all_batches_of_tracked_files() {
    let (_tmp, repo, run) = fixture();
    for n in 0..260 {
        fs::write(repo.join(format!("file-{n:03}")), format!("value-{n:03}")).unwrap();
    }
    command(&repo, &["add", "--all"]);
    command(&repo, &["commit", "-qm", "multiple batches"]);
    let w = Worktree::create(&repo, &run).unwrap();
    w.verify_candidate(&w.baseline, &w.baseline).unwrap();
    fs::write(w.path.join("file-255"), "different").unwrap();
    assert!(w.verify_candidate(&w.baseline, &w.baseline).is_err());
}

#[test]
fn managed_worktree_rejects_ignored_files_but_source_ignored_files_are_preserved() {
    let (_tmp, repo, run) = fixture();
    fs::write(repo.join(".gitignore"), "ignored.tmp\n").unwrap();
    command(&repo, &["add", ".gitignore"]);
    command(&repo, &["commit", "-qm", "ignore local files"]);
    fs::write(repo.join("ignored.tmp"), "user-owned ignored data").unwrap();
    let w = Worktree::create(&repo, &run).unwrap();
    assert!(!w.path.join("ignored.tmp").exists());
    w.verify_candidate(&w.baseline, &w.baseline).unwrap();
    fs::write(w.path.join("ignored.tmp"), "unreviewed runtime data").unwrap();
    assert!(
        w.verify_candidate(&w.baseline, &w.baseline).is_err(),
        "ignored content in an active worktree must not survive a verified checkpoint"
    );
    assert!(w.verify_clean().is_err());
    assert_eq!(
        fs::read_to_string(repo.join("ignored.tmp")).unwrap(),
        "user-owned ignored data"
    );
}

#[test]
fn empty_ignored_directories_are_allowed_but_files_and_symlinks_are_not() {
    let (_tmp, repo, run) = fixture();
    fs::write(repo.join(".gitignore"), "cache/\n").unwrap();
    command(&repo, &["add", ".gitignore"]);
    command(&repo, &["commit", "-qm", "ignore cache"]);
    let w = Worktree::create(&repo, &run).unwrap();
    fs::create_dir_all(w.path.join("cache/nested")).unwrap();
    w.verify_candidate(&w.baseline, &w.baseline).unwrap();
    fs::write(w.path.join("cache/nested/data"), "not reviewed").unwrap();
    assert!(w.verify_candidate(&w.baseline, &w.baseline).is_err());
    fs::remove_file(w.path.join("cache/nested/data")).unwrap();
    std::os::unix::fs::symlink("nested", w.path.join("cache/link")).unwrap();
    assert!(w.verify_candidate(&w.baseline, &w.baseline).is_err());
}

#[test]
fn worktree_is_not_created_inside_separate_git_common_directory() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let common = temp.path().join("external.git");
    fs::create_dir(&repo).unwrap();
    command(
        &repo,
        &["init", "--separate-git-dir", common.to_str().unwrap(), "-q"],
    );
    command(&repo, &["config", "user.email", "test@example.invalid"]);
    command(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("data"), "baseline").unwrap();
    command(&repo, &["add", "data"]);
    command(&repo, &["commit", "-qm", "baseline"]);
    let run = common.join("unexpected-run");
    assert!(
        simon_says::git::InitializationIntent::capture(&repo, &run).is_err(),
        "run directory must not be placed inside Git's common directory"
    );
    assert!(!run.exists());
    assert_eq!(fs::read_to_string(repo.join("data")).unwrap(), "baseline");
}
