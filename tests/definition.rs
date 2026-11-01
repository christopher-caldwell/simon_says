use simon_says::{Snapshot, Workflow};
use std::{fs, path::Path};

fn setup() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    for file in [
        "reviewer.md",
        "unblocker.md",
        "validate.md",
        "report.md",
        "report.schema.json",
    ] {
        fs::copy(
            Path::new("examples/basic").join(file),
            dir.path().join(file),
        )
        .unwrap();
    }
    (
        dir,
        fs::read_to_string("examples/basic/workflow.toml").unwrap(),
    )
}
#[test]
fn valid_two_stage_template() {
    let (d, t) = setup();
    let (_, refs) = Workflow::parse_and_validate(&t, d.path()).unwrap();
    assert_eq!(refs.len(), 5);
}
#[test]
fn validates_bounded_named_routes_and_rejects_invalid_graphs() {
    let (d, t) = setup();
    let routed = t
        .replace(
            "start = \"validate\"",
            "start = \"validate\"\nmax_stage_transitions = 4",
        )
        .replace(
            "on_pass = \"report\"",
            "on_pass = \"report\"\nroutes = { issues = \"report\", clean = \"complete\" }",
        )
        .replace("on_pass = \"complete\"", "on_pass = \"validate\"");
    Workflow::parse_and_validate(&routed, d.path()).unwrap();
    for bad in [
        routed.replace("max_stage_transitions = 4\n", ""),
        routed.replace("max_stage_transitions = 4", "max_stage_transitions = 0"),
        routed.replace("max_stage_transitions = 4", "max_stage_transitions = 1001"),
        routed.replace("issues = \"report\"", "issues = \"missing\""),
        routed.replace("issues = \"report\"", "\"bad label\" = \"report\""),
        routed.replace("on_pass = \"report\"", "on_pass = \"validate\""),
        routed.replace("clean = \"complete\"", "clean = \"validate\""),
    ] {
        assert!(
            Workflow::parse_and_validate(&bad, d.path()).is_err(),
            "accepted {bad}"
        );
    }
}
#[test]
fn rejects_unknown_fields_roles_and_routes() {
    let (d, t) = setup();
    for bad in [
        format!("unexpected = true\n{t}"),
        t.replace("worker = \"worker\"", "worker = \"missing\""),
        t.replace("on_pass = \"report\"", "on_pass = \"unknown\""),
    ] {
        assert!(Workflow::parse_and_validate(&bad, d.path()).is_err());
    }
}
#[test]
fn rejects_cycles_unreachable_stages_and_escaping_paths() {
    let (d, t) = setup();
    for bad in [
        t.replace("on_pass = \"complete\"", "on_pass = \"validate\""),
        t.replace("on_pass = \"report\"", "on_pass = \"complete\""),
        t.replace("validate.md", "../validate.md"),
        t.replace("data/input.json", "/tmp/input.json"),
    ] {
        assert!(Workflow::parse_and_validate(&bad, d.path()).is_err());
    }
}
#[test]
fn rejects_missing_or_invalid_authority() {
    let (d, t) = setup();
    fs::remove_file(d.path().join("report.md")).unwrap();
    assert!(Workflow::parse_and_validate(&t, d.path()).is_err());
    fs::write(d.path().join("report.md"), "report").unwrap();
    fs::write(d.path().join("report.schema.json"), "not json").unwrap();
    assert!(Workflow::parse_and_validate(&t, d.path()).is_err());
}
#[cfg(unix)]
#[test]
fn rejects_authoritative_symlinks() {
    let (d, t) = setup();
    fs::remove_file(d.path().join("report.md")).unwrap();
    std::os::unix::fs::symlink("validate.md", d.path().join("report.md")).unwrap();
    assert!(Workflow::parse_and_validate(&t, d.path()).is_err());
}
#[test]
fn snapshot_is_self_contained_and_detects_mutation() {
    let (d, t) = setup();
    let template = d.path().join("workflow.toml");
    fs::write(&template, t).unwrap();
    let mut snap = Snapshot::create(&template).unwrap();
    fs::write(d.path().join("report.md"), "changed").unwrap();
    snap.verify().unwrap();
    assert_ne!(snap.sha256, Snapshot::create(&template).unwrap().sha256);
    snap.files.insert("report.md".into(), "tampered".into());
    assert!(snap.verify().is_err());
}
#[test]
fn snapshot_never_overwrites_existing_file() {
    let (d, t) = setup();
    let template = d.path().join("workflow.toml");
    fs::write(&template, t).unwrap();
    let s = Snapshot::create(&template).unwrap();
    let out = d.path().join("snapshot.json");
    s.write_new(&out).unwrap();
    assert!(s.write_new(&out).is_err());
    simon_says::load_snapshot(&out).unwrap();
}
#[test]
fn rejects_invalid_schemas_external_refs_and_git_paths() {
    let (d, t) = setup();
    fs::write(d.path().join("report.schema.json"), r#"{"type":123}"#).unwrap();
    assert!(Workflow::parse_and_validate(&t, d.path()).is_err());
    fs::write(
        d.path().join("report.schema.json"),
        r##"{"$ref":"other.json#/x"}"##,
    )
    .unwrap();
    assert!(Workflow::parse_and_validate(&t, d.path()).is_err());
    fs::write(d.path().join("report.schema.json"), "{}").unwrap();
    assert!(
        Workflow::parse_and_validate(&t.replace("data/input.json", ".git/config"), d.path())
            .is_err()
    );
}
#[test]
fn rejects_unsupported_schema_drafts_and_unresolved_references() {
    let (d, t) = setup();
    for schema in [
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","prefixItems":[{"type":"integer"}]}"#,
        r##"{"$ref":"#/missing"}"##,
    ] {
        fs::write(d.path().join("report.schema.json"), schema).unwrap();
        assert!(Workflow::parse_and_validate(&t, d.path()).is_err());
    }
}
#[cfg(unix)]
#[test]
fn rejects_symlinked_template() {
    let (d, t) = setup();
    fs::write(d.path().join("real.toml"), t).unwrap();
    std::os::unix::fs::symlink("real.toml", d.path().join("link.toml")).unwrap();
    assert!(Snapshot::create(&d.path().join("link.toml")).is_err());
}
#[test]
fn rejects_unbounded_action_timeouts() {
    let (d, t) = setup();
    for seconds in [0, 86401] {
        let bad = t.replace(
            "kind = \"command\"",
            &format!("kind = \"command\"\ntimeout_seconds = {seconds}"),
        );
        assert!(Workflow::parse_and_validate(&bad, d.path()).is_err());
    }
}
#[test]
fn run_context_is_bounded_utf8_and_digest_bound() {
    let (d, t) = setup();
    let template = d.path().join("workflow.toml");
    fs::write(&template, t).unwrap();
    let context = d.path().join("context.txt");
    fs::write(&context, "original task and authority").unwrap();
    let original = Snapshot::create(&template).unwrap();
    let frozen = Snapshot::create(&template)
        .unwrap()
        .with_context(&context)
        .unwrap();
    assert_ne!(original.sha256, frozen.sha256);
    frozen.verify().unwrap();
    let encoded = serde_json::to_vec(&original).unwrap();
    let old: Snapshot = serde_json::from_slice(&encoded).unwrap();
    old.verify().unwrap();
    let mut tampered = frozen;
    tampered.run_context = Some("changed".into());
    assert!(tampered.verify().is_err());
    fs::write(&context, vec![b'x'; 32769]).unwrap();
    assert!(Snapshot::create(&template)
        .unwrap()
        .with_context(&context)
        .is_err());
    fs::write(&context, [0xff]).unwrap();
    assert!(Snapshot::create(&template)
        .unwrap()
        .with_context(&context)
        .is_err());
}
#[cfg(unix)]
#[test]
fn rejects_symlinked_run_context() {
    let (d, t) = setup();
    let template = d.path().join("workflow.toml");
    fs::write(&template, t).unwrap();
    fs::write(d.path().join("context"), "task").unwrap();
    std::os::unix::fs::symlink("context", d.path().join("link")).unwrap();
    assert!(Snapshot::create(&template)
        .unwrap()
        .with_context(&d.path().join("link"))
        .is_err());
}
#[test]
fn frozen_project_paths_cannot_escape_duplicate_or_be_outputs() {
    let (d, t) = setup();
    for values in [
        "[\"../unsafe\"]",
        "[\"x\",\"x\"]",
        "[\"reports/validation.json\"]",
    ] {
        let bad = t.replacen(
            "schema_version = 1",
            &format!("schema_version = 1\nfrozen_project_files = {values}"),
            1,
        );
        assert!(Workflow::parse_and_validate(&bad, d.path()).is_err());
    }
}

#[test]
fn session_continuation_is_opt_in_and_valid_only_for_non_ephemeral_codex_agents() {
    let (d, t) = setup();
    let allowed = t.replace(
        "profile = \"reviewer.md\"",
        "profile = \"reviewer.md\"\nresume_worker_session = true",
    );
    Workflow::parse_and_validate(&allowed, d.path()).unwrap();
    let cases = [
        t.replace(
            "argv = [\"./worker\"]",
            "argv = [\"./worker\"]\nresume_worker_session = true",
        ),
        t.replace(
            "profile = \"unblocker.md\"",
            "profile = \"unblocker.md\"\nresume_worker_session = true",
        ),
        allowed.replace(
            "resume_worker_session = true",
            "resume_worker_session = true\nargs = [\"--ephemeral\"]",
        ),
        allowed.replace(
            "resume_worker_session = true",
            "resume_worker_session = true\nargs = [\"--ephemeral=true\"]",
        ),
    ];
    for bad in cases {
        assert!(Workflow::parse_and_validate(&bad, d.path()).is_err());
    }
}

#[test]
fn command_role_errors_identify_agent_only_fields() {
    let (root, base) = setup();
    for (field, value) in [
        ("adapter", "\"codex\""),
        ("model", "\"example-model\""),
        ("effort", "\"low\""),
        ("profile", "\"reviewer.md\""),
        ("args", "[\"--example\"]"),
    ] {
        let text = base.replace(
            "argv = [\"./worker\"]",
            &format!("argv = [\"./worker\"]\n{field} = {value}"),
        );
        let error = Workflow::parse_and_validate(&text, root.path()).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("role worker") && message.contains(field),
            "{message}"
        );
    }
    let empty = base.replace("argv = [\"./worker\"]", "argv = []");
    let error = Workflow::parse_and_validate(&empty, root.path()).unwrap_err();
    assert!(format!("{error:#}").contains("nonempty argv"));
}
