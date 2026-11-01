use simon_says::sandbox;
#[cfg(target_os = "macos")]
mod macos {
    use super::sandbox;
    use std::{fs, path::Path, process::Command};
    fn run(plan: &sandbox::SandboxPlan, args: &[(&str, &Path)]) -> std::process::Output {
        let mut c = Command::new(&plan.program);
        c.args(&plan.argv);
        for (k, v) in args {
            c.env(k, v);
        }
        c.output().unwrap()
    }
    fn fixture() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap().join("run");
        let cwd = root.join("worktree");
        let scratch = root.join("actions/action-0/scratch");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(&scratch).unwrap();
        fs::write(root.join("state.json"), "baseline").unwrap();
        fs::write(root.join("snapshot.json"), "authority").unwrap();
        (t, root, cwd, scratch)
    }
    #[test]
    fn direct_writes_and_renaming_run_are_denied_but_checkout_and_scratch_work() {
        let (_t, r, c, s) = fixture();
        let script="echo good > \"$CWD/allowed\"; echo scratch > \"$SCRATCH/out\"; echo bad > \"$RUN/state.json\" 2>/dev/null && exit 9; mv \"$RUN\" \"$RUN-moved\" 2>/dev/null && exit 10; exit 0";
        let p = sandbox::prepare(&r, &c, &s, "/bin/sh", &["-c".into(), script.into()]).unwrap();
        let result = run(&p, &[("RUN", &r), ("CWD", &c), ("SCRATCH", &s)]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read_to_string(r.join("state.json")).unwrap(),
            "baseline"
        );
        assert!(c.join("allowed").exists());
        assert!(s.join("out").exists());
    }
    #[test]
    fn symlink_and_hardlink_cannot_write_protected_authority() {
        let (_t, r, c, s) = fixture();
        std::os::unix::fs::symlink(r.join("snapshot.json"), c.join("link")).unwrap();
        let script="echo bad > \"$CWD/link\" 2>/dev/null && exit 4; ln \"$RUN/snapshot.json\" \"$CWD/hard\" 2>/dev/null && { echo bad > \"$CWD/hard\"; exit 5; }; exit 0";
        let p = sandbox::prepare(&r, &c, &s, "/bin/sh", &["-c".into(), script.into()]).unwrap();
        let result = run(&p, &[("RUN", &r), ("CWD", &c)]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read_to_string(r.join("snapshot.json")).unwrap(),
            "authority"
        );
    }
    #[test]
    fn linked_worktree_can_commit() {
        let (_t, r, c, s) = fixture();
        let project = r.parent().unwrap().join("source");
        fs::create_dir(&project).unwrap();
        let git = |path: &Path, args: &[&str]| {
            let o = Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        };
        git(&project, &["init", "-q"]);
        git(&project, &["config", "user.name", "test"]);
        git(&project, &["config", "user.email", "test@example.invalid"]);
        fs::write(project.join("a"), "one").unwrap();
        git(&project, &["add", "a"]);
        git(&project, &["commit", "-qm", "baseline"]);
        fs::remove_dir(&c).unwrap();
        git(
            &project,
            &["worktree", "add", "--detach", c.to_str().unwrap(), "HEAD"],
        );
        let script="echo two > a && git add a && git -c user.name=test -c user.email=test@example.invalid commit -qm candidate";
        let p = sandbox::prepare(
            &r,
            &c,
            &s,
            "/bin/sh",
            &["-c".into(), format!("cd \"$CWD\" && {script}")],
        )
        .unwrap();
        let result = run(&p, &[("CWD", &c)]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read_to_string(r.join("snapshot.json")).unwrap(),
            "authority"
        );
    }
    #[test]
    fn rejects_wrong_or_symlinked_scratch_and_other_checkouts() {
        let (_t, r, c, s) = fixture();
        let other = r.join("other");
        fs::create_dir(&other).unwrap();
        assert!(sandbox::prepare(&r, &other, &s, "/bin/true", &[]).is_err());
        let wrong = r.join("actions/action-1/escape");
        fs::create_dir_all(&wrong).unwrap();
        assert!(sandbox::prepare(&r, &c, &wrong, "/bin/true", &[]).is_err());
        let link = r.join("actions/action-2/scratch");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&s, &link).unwrap();
        assert!(sandbox::prepare(&r, &c, &link, "/bin/true", &[]).is_err());
    }
}
#[cfg(target_os = "macos")]
#[test]
fn nested_probe_matches_real_capability_of_the_exact_outer_profile() {
    use std::{fs, process::Command};
    let tmp = tempfile::tempdir().unwrap();
    let run = tmp.path().canonicalize().unwrap().join("run");
    let cwd = run.join("worktree");
    let scratch = run.join("actions/action-0/scratch");
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(&scratch).unwrap();
    let plan = sandbox::prepare(&run, &cwd, &scratch, "/usr/bin/true", &[]).unwrap();
    let direct = Command::new("/usr/bin/sandbox-exec")
        .args([
            "-p",
            &plan.argv[1],
            "--",
            "/usr/bin/sandbox-exec",
            "-p",
            "(version 1)(allow default)",
            "--",
            "/usr/bin/true",
        ])
        .output()
        .unwrap();
    let probed = sandbox::probe_nested(&plan);
    assert_eq!(probed.is_ok(), direct.status.success());
    if let Err(e) = probed {
        assert!(e.to_string().contains("no model was invoked"));
    }
    let mut forged = plan;
    forged.program = "/bin/sh".into();
    assert!(sandbox::probe_nested(&forged).is_err());
}
#[cfg(target_os = "macos")]
#[test]
fn frozen_project_file_rejects_direct_write_during_action() {
    use std::{fs, process::Command};
    let tmp = tempfile::tempdir().unwrap();
    let run = tmp.path().canonicalize().unwrap().join("run");
    let cwd = run.join("worktree");
    let scratch = run.join("actions/action-0/scratch");
    fs::create_dir_all(&cwd).unwrap();
    fs::create_dir_all(&scratch).unwrap();
    fs::write(cwd.join("verifier.py"), "original").unwrap();
    let plan = sandbox::prepare_with_frozen(
        &run,
        &cwd,
        &scratch,
        "/bin/sh",
        &["-c".into(), "printf changed > verifier.py".into()],
        &["verifier.py".into()],
    )
    .unwrap();
    let result = Command::new(plan.program)
        .args(plan.argv)
        .current_dir(&cwd)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(
        fs::read_to_string(cwd.join("verifier.py")).unwrap(),
        "original"
    );
}
#[cfg(target_os = "macos")]
#[test]
fn frozen_file_and_all_ancestors_resist_rename_and_replacement() {
    use std::{fs, process::Command};
    let cases = [
        ("direct write", "printf changed > \"$FROZEN\"", false),
        ("unlink", "rm \"$FROZEN\"", false),
        ("atomic replacement", "printf changed > \"$CWD/nest/deep/tmp\" && mv -f \"$CWD/nest/deep/tmp\" \"$FROZEN\"", false),
        ("hardlink", "ln \"$FROZEN\" \"$CWD/nest/deep/hard\" && printf changed > \"$CWD/nest/deep/hard\"", false),
        ("symlink", "ln -s \"$FROZEN\" \"$CWD/nest/deep/link\" && printf changed > \"$CWD/nest/deep/link\"", false),
        ("inner ancestor rename", "mv \"$CWD/nest/deep\" \"$CWD/nest/moved\" && printf changed > \"$CWD/nest/moved/guide \\\"safety\\\".md\"", false),
        ("outer ancestor rename", "mv \"$CWD/nest\" \"$CWD/moved\" && printf changed > \"$CWD/moved/deep/guide \\\"safety\\\".md\"", false),
        ("checkout root rename", "mv \"$CWD\" \"$RUN-moved\" && printf changed > \"$RUN-moved/nest/deep/guide \\\"safety\\\".md\"", false),
        ("sibling edits", "printf changed > \"$CWD/nest/deep/sibling\" && rm \"$CWD/nest/deep/sibling\" && mkdir \"$CWD/nest/deep/temporary\" && rmdir \"$CWD/nest/deep/temporary\"", true),
    ];
    for (name, script, allowed) in cases {
        let tmp = tempfile::tempdir().unwrap();
        let run = tmp.path().canonicalize().unwrap().join("run");
        let cwd = run.join("worktree");
        let scratch = run.join("actions/action-0/scratch");
        fs::create_dir_all(cwd.join("nest/deep")).unwrap();
        fs::create_dir_all(&scratch).unwrap();
        let relative = "nest/deep/guide \"safety\".md";
        let file = cwd.join(relative);
        fs::write(&file, "original").unwrap();
        let plan = sandbox::prepare_with_frozen(
            &run,
            &cwd,
            &scratch,
            "/bin/sh",
            &["-c".into(), script.into()],
            &[relative.into()],
        )
        .unwrap();
        let output = Command::new(&plan.program)
            .args(&plan.argv)
            .current_dir(&cwd)
            .env("CWD", &cwd)
            .env("RUN", &run)
            .env("FROZEN", &file)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            allowed,
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "original",
            "{name} altered authority"
        );
    }
}
#[cfg(target_os = "macos")]
#[test]
fn frozen_file_refuses_symlinked_parents_and_control_characters() {
    use std::{fs, os::unix::fs::symlink};
    let tmp = tempfile::tempdir().unwrap();
    let run = tmp.path().canonicalize().unwrap().join("run");
    let cwd = run.join("worktree");
    let scratch = run.join("actions/action-0/scratch");
    fs::create_dir_all(cwd.join("real")).unwrap();
    fs::create_dir_all(&scratch).unwrap();
    fs::write(cwd.join("real/file"), "authority").unwrap();
    symlink("real", cwd.join("link")).unwrap();
    assert!(sandbox::prepare_with_frozen(
        &run,
        &cwd,
        &scratch,
        "/bin/true",
        &[],
        &["link/file".into()]
    )
    .is_err());
    fs::write(cwd.join("line\nbreak"), "authority").unwrap();
    assert!(sandbox::prepare_with_frozen(
        &run,
        &cwd,
        &scratch,
        "/bin/true",
        &[],
        &["line\nbreak".into()]
    )
    .is_err());
}
#[cfg(target_os = "macos")]
#[test]
fn frozen_authority_allows_git_commit_of_other_outputs() {
    use std::{fs, process::Command};
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let source = root.join("source");
    let run = root.join("run");
    let cwd = run.join("worktree");
    let scratch = run.join("actions/action-0/scratch");
    fs::create_dir_all(source.join("policy")).unwrap();
    fs::create_dir_all(&scratch).unwrap();
    let git = |cwd: &std::path::Path, args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&source, &["init", "-q"]);
    git(&source, &["config", "user.name", "Fixture"]);
    git(
        &source,
        &["config", "user.email", "fixture@example.invalid"],
    );
    fs::write(source.join("policy/authority.md"), "fixed").unwrap();
    fs::write(source.join("output.txt"), "old").unwrap();
    git(&source, &["add", "."]);
    git(&source, &["commit", "-qm", "baseline"]);
    git(
        &source,
        &["worktree", "add", "--detach", cwd.to_str().unwrap(), "HEAD"],
    );
    let script="printf result > output.txt && git add output.txt && git -c user.name=Fixture -c user.email=fixture@example.invalid commit -qm output";
    let plan = sandbox::prepare_with_frozen(
        &run,
        &cwd,
        &scratch,
        "/bin/sh",
        &["-c".into(), script.into()],
        &["policy/authority.md".into()],
    )
    .unwrap();
    let output = Command::new(plan.program)
        .args(plan.argv)
        .current_dir(&cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(cwd.join("policy/authority.md")).unwrap(),
        "fixed"
    );
    assert_eq!(
        fs::read_to_string(cwd.join("output.txt")).unwrap(),
        "result"
    );
}
