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
