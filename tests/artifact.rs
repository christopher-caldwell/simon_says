use simon_says::artifact::{frozen_paths, tracked_blob};
use std::{fs, path::Path, process::Command};

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn fixture() -> (tempfile::TempDir, std::path::PathBuf, String) {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Fixture"]);
    git(&repo, &["config", "user.email", "fixture@example.invalid"]);
    git(&repo, &["config", "core.filemode", "true"]);
    fs::create_dir(repo.join("nested")).unwrap();
    fs::write(repo.join("nested/authority.md"), "original authority\n").unwrap();
    fs::write(repo.join("other.txt"), "baseline\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "baseline"]);
    let baseline = git(&repo, &["rev-parse", "HEAD"]);
    (root, repo, baseline)
}
fn commit(repo: &Path) -> String {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-qm", "candidate"]);
    git(repo, &["rev-parse", "HEAD"])
}
fn paths() -> Vec<String> {
    vec!["nested/authority.md".into()]
}
#[test]
fn accepts_exact_authority_across_unrelated_candidate_change() {
    let (_root, repo, base) = fixture();
    fs::write(repo.join("other.txt"), "candidate\n").unwrap();
    let candidate = commit(&repo);
    frozen_paths(&repo, &base, &candidate, &paths()).unwrap();
    assert_eq!(
        tracked_blob(&repo, &candidate, "nested/authority.md").unwrap(),
        b"original authority\n"
    );
}
#[test]
fn rejects_content_change_and_deletion() {
    let (_root, repo, base) = fixture();
    fs::write(repo.join("nested/authority.md"), "edited\n").unwrap();
    let changed = commit(&repo);
    assert!(frozen_paths(&repo, &base, &changed, &paths()).is_err());
    fs::remove_file(repo.join("nested/authority.md")).unwrap();
    let deleted = commit(&repo);
    assert!(frozen_paths(&repo, &base, &deleted, &paths()).is_err());
}
#[cfg(unix)]
#[test]
fn rejects_mode_change_and_symlink_substitution() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let (_root, repo, base) = fixture();
    let file = repo.join("nested/authority.md");
    let mut permissions = fs::metadata(&file).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&file, permissions).unwrap();
    let mode = commit(&repo);
    assert!(frozen_paths(&repo, &base, &mode, &paths()).is_err());
    fs::remove_file(&file).unwrap();
    symlink("../other.txt", &file).unwrap();
    let link = commit(&repo);
    assert!(frozen_paths(&repo, &base, &link, &paths()).is_err());
    assert!(frozen_paths(&repo, &link, &link, &paths()).is_err());
}
#[test]
fn rejects_gitlink_and_noncommit_or_ambiguous_commit_names() {
    let (_root, repo, base) = fixture();
    git(
        &repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{base},vendor"),
        ],
    );
    git(&repo, &["commit", "-qm", "add gitlink"]);
    let candidate = git(&repo, &["rev-parse", "HEAD"]);
    assert!(frozen_paths(&repo, &candidate, &candidate, &["vendor".into()]).is_err());
    assert!(frozen_paths(&repo, "HEAD", &candidate, &paths()).is_err());
}
#[test]
fn treats_git_pathspec_metacharacters_and_tabs_as_literal_names() {
    let (_root, repo, _base) = fixture();
    fs::write(repo.join("nested/a*.md"), "literal").unwrap();
    fs::write(repo.join("nested/aaa.md"), "other").unwrap();
    fs::write(repo.join("nested/tab\tname"), "tab").unwrap();
    let base = commit(&repo);
    let files = vec!["nested/a*.md".into(), "nested/tab\tname".into()];
    frozen_paths(&repo, &base, &base, &files).unwrap();
    assert_eq!(
        tracked_blob(&repo, &base, "nested/a*.md").unwrap(),
        b"literal"
    );
    assert!(frozen_paths(&repo, &base, &base, &["../other.txt".into()]).is_err());
}
