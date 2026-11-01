use simon_says::{provider, Workflow};
use std::{fs, path::Path};

fn template() -> (tempfile::TempDir, String) {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "reviewer.md",
        "unblocker.md",
        "validate.md",
        "report.md",
        "report.schema.json",
    ] {
        fs::copy(
            Path::new("examples/basic").join(name),
            root.path().join(name),
        )
        .unwrap();
    }
    (
        root,
        fs::read_to_string("examples/basic/workflow.toml").unwrap(),
    )
}

#[test]
fn workflow_validates_confined_scratch_environment_bindings() {
    let (root, base) = template();
    let with = |item: &str| format!("{base}\n[roles.worker.scratch_env]\n{item}\n");
    let valid =
        with("CARGO_TARGET_DIR = \"compiler/cache\"\nPYTHONPYCACHEPREFIX = \"python-cache\"");
    let (workflow, _) = Workflow::parse_and_validate(&valid, root.path()).unwrap();
    assert_eq!(
        workflow.roles["worker"].scratch_env["CARGO_TARGET_DIR"],
        "compiler/cache"
    );
    for invalid in [
        "PATH = \"cache\"",
        "HOME = \"cache\"",
        "SIMON_PACKET = \"cache\"",
        "GIT_DIR = \"cache\"",
        "DYLD_INSERT_LIBRARIES = \"cache\"",
        "PYTHONPATH = \"cache\"",
        "NODE_OPTIONS = \"cache\"",
        "\"not-valid\" = \"cache\"",
        "lowercase = \"cache\"",
        "\"9START\" = \"cache\"",
        "CUSTOM_CACHE = \"/absolute\"",
        "CUSTOM_CACHE = \"../outside\"",
        "CUSTOM_CACHE = \"a/../b\"",
        "CUSTOM_CACHE = \"a/./b\"",
        "CUSTOM_CACHE = \"a//b\"",
        "CUSTOM_CACHE = \".git/config\"",
        "CUSTOM_CACHE = \"\"",
    ] {
        assert!(
            Workflow::parse_and_validate(&with(invalid), root.path()).is_err(),
            "accepted {invalid}"
        );
    }
}

#[cfg(unix)]
#[test]
fn scratch_binding_rejects_symlink_components() {
    let (root, base) = template();
    let text = format!("{base}\n[roles.worker.scratch_env]\nCUSTOM_CACHE = \"cache/child\"\n");
    let (workflow, _) = Workflow::parse_and_validate(&text, root.path()).unwrap();
    let scratch = root.path().join("scratch");
    let outside = root.path().join("outside");
    fs::create_dir(&scratch).unwrap();
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, scratch.join("cache")).unwrap();
    assert!(provider::scratch_environment(&workflow.roles["worker"], &scratch).is_err());
    assert!(!outside.join("child").exists());
}

#[cfg(target_os = "macos")]
#[test]
fn controller_passes_distinct_recorded_scratch_paths_to_each_role() {
    use std::process::Command;
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    for entry in fs::read_dir("examples/dynamic-repair").unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), repo.join(entry.file_name())).unwrap();
    }
    let mini = repo.join("mini");
    fs::create_dir_all(mini.join("src")).unwrap();
    fs::write(
        mini.join("Cargo.toml"),
        "[package]\nname = \"mini-cache-check\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(
        mini.join("Cargo.lock"),
        "version = 4\n\n[[package]]\nname = \"mini-cache-check\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(mini.join("src/lib.rs"), "pub fn check() -> u32 { 42 }\n").unwrap();
    let workflow = repo.join("workflow.toml");
    let mut text = fs::read_to_string(&workflow).unwrap();
    text.push_str("\n[roles.simulator.scratch_env]\nCARGO_TARGET_DIR = \"compiler/cache\"\n[roles.quality_reviewer.scratch_env]\nCARGO_TARGET_DIR = \"compiler/cache\"\n");
    fs::write(&workflow, text).unwrap();
    fs::write(repo.join("engine.json"), "{\"correct\":true}\n").unwrap();
    let action = repo.join("action.py");
    let text = fs::read_to_string(&action).unwrap();
    let needle = "    packet = read(os.environ[\"SIMON_PACKET\"])\n";
    let replacement = "    packet = read(os.environ[\"SIMON_PACKET\"])\n    target = Path(os.environ[\"CARGO_TARGET_DIR\"])\n    assert str(target) == packet[\"scratch_env\"][\"CARGO_TARGET_DIR\"]\n    subprocess.run([\"cargo\", \"check\", \"--locked\", \"--offline\", \"--manifest-path\", \"mini/Cargo.toml\"], check=True, stdout=subprocess.DEVNULL)\n    (target / \"marker\").write_text(packet[\"action_id\"])\n";
    assert_eq!(text.matches(needle).count(), 1);
    fs::write(action, text.replace(needle, replacement)).unwrap();
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "-q"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    git(&["add", "."]);
    git(&["commit", "-qm", "baseline"]);
    let run = root.path().join("run");
    let inherited = root.path().join("inherited");
    let output = Command::new(env!("CARGO_BIN_EXE_simon-says"))
        .args([
            "run",
            workflow.to_str().unwrap(),
            "--project",
            repo.to_str().unwrap(),
            "--run-dir",
            run.to_str().unwrap(),
        ])
        .env("CARGO_TARGET_DIR", &inherited)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut targets = Vec::new();
    for index in 0..2 {
        let dir = run.join(format!("actions/action-{index}"));
        let packet: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("packet.json")).unwrap()).unwrap();
        let plan: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("invocation.json")).unwrap()).unwrap();
        let target = packet["scratch_env"]["CARGO_TARGET_DIR"].as_str().unwrap();
        assert_eq!(plan["env"]["CARGO_TARGET_DIR"], target);
        assert!(Path::new(target).join("debug").is_dir());
        assert_eq!(
            fs::read_to_string(Path::new(target).join("marker")).unwrap(),
            format!("action-{index}")
        );
        assert!(Path::new(target).starts_with(dir.join("scratch").canonicalize().unwrap()));
        targets.push(target.to_string());
    }
    assert_ne!(targets[0], targets[1]);
    assert!(!inherited.exists());
    assert!(!run.join("worktree/compiler").exists());
    assert!(!run.join("worktree/mini/target").exists());
}

#[test]
fn legacy_invocation_without_environment_deserializes_and_forged_env_fails_before_spawn() {
    use simon_says::routing::Gate;
    let (root, base) = template();
    let (workflow, _) = Workflow::parse_and_validate(&base, root.path()).unwrap();
    let role = &workflow.roles["worker"];
    let scratch = root.path().join("scratch");
    fs::create_dir(&scratch).unwrap();
    let packet = root.path().join("packet.json");
    let mut plan = provider::prepare(role, Gate::Worker, "", root.path(), &packet).unwrap();
    plan.scratch = Some(scratch.clone());
    let mut encoded = serde_json::to_value(&plan).unwrap();
    encoded.as_object_mut().unwrap().remove("env");
    let legacy: provider::InvocationPlan = serde_json::from_value(encoded).unwrap();
    assert!(legacy.env.is_empty());
    let outside = root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    plan.env.insert("CUSTOM_CACHE".into(), outside);
    assert!(provider::invoke(&plan, root.path()).is_err());
    assert!(!root.path().join("stdout.txt").exists());
    plan.env.clear();
    let child = scratch.join("child");
    fs::create_dir(&child).unwrap();
    plan.env.insert("SIMON_PACKET".into(), child);
    assert!(provider::invoke(&plan, root.path()).is_err());
    assert!(!root.path().join("stdout.txt").exists());
}
