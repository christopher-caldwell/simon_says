#[path = "../src/sandbox.rs"]
mod sandbox;
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
