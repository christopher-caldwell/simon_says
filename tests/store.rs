use simon_says::{
    routing::{State, Status},
    store::{RunRecord, RunStore},
    Snapshot, Workflow,
};
use std::{fs, path::Path};
fn init() -> (tempfile::TempDir, RunRecord, RunStore) {
    let root = tempfile::tempdir().unwrap();
    let snapshot = Snapshot::create(Path::new("examples/basic/workflow.toml")).unwrap();
    let (w, _) =
        Workflow::parse_and_validate(&snapshot.template, Path::new("examples/basic")).unwrap();
    let record = RunRecord {
        version: 1,
        run_id: "test".into(),
        snapshot_digest: snapshot.sha256.clone(),
        state: State::new(&w, "abc".into()),
        next_action: 0,
        worktree: serde_json::Value::Null,
    };
    let store = RunStore::create(&root.path().join("run"), &snapshot, &record).unwrap();
    (root, record, store)
}
#[test]
fn running_is_persisted_before_invocation_and_restart_does_not_replay() {
    let (_tmp, mut record, store) = init();
    let packet = serde_json::json!({"action_id":"action-0"});
    store.prepare_action(&mut record, "action-0").unwrap();
    let dir = store
        .begin(
            &mut record,
            &packet,
            &serde_json::json!({"argv":["worker"]}),
        )
        .unwrap();
    assert!(dir.join("packet.json").exists());
    assert!(dir.join("invocation.json").exists());
    let root = store.root().to_owned();
    drop(store);
    let store = RunStore::open(&root).unwrap();
    let (loaded, _) = store.load().unwrap();
    assert_eq!(loaded.state.status, Status::Running);
    assert!(RunStore::check_resumable(&loaded).is_err());
}
#[test]
fn exclusive_lock_and_snapshot_anchor_are_checked() {
    let (_tmp, _, store) = init();
    let root = store.root().to_owned();
    assert!(RunStore::open(&root).is_err());
    let mut state: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("state.json")).unwrap()).unwrap();
    state["snapshot_digest"] = "wrong".into();
    fs::write(root.join("state.json"), serde_json::to_vec(&state).unwrap()).unwrap();
    assert!(store.load().is_err());
}
#[test]
fn existing_run_is_never_overwritten() {
    let (_tmp, record, store) = init();
    let root = store.root().to_owned();
    let s = Snapshot::create(Path::new("examples/basic/workflow.toml")).unwrap();
    assert!(RunStore::create(&root, &s, &record).is_err());
}
#[test]
fn preparation_is_durable_and_matching_identity_starts_once() {
    let (_tmp, mut record, store) = init();
    store.prepare_action(&mut record, "action-0").unwrap();
    assert_eq!(record.state.status, Status::Preparing);
    assert_eq!(record.next_action, 1);
    let root = store.root().to_owned();
    drop(store);
    let store = RunStore::open(&root).unwrap();
    let (mut loaded, _) = store.load().unwrap();
    assert_eq!(loaded.state.action_id.as_deref(), Some("action-0"));
    assert!(RunStore::check_resumable(&loaded).is_err());
    assert!(store
        .begin(
            &mut loaded,
            &serde_json::json!({"action_id":"action-1"}),
            &serde_json::json!({})
        )
        .is_err());
    store
        .begin(
            &mut loaded,
            &serde_json::json!({"action_id":"action-0"}),
            &serde_json::json!({}),
        )
        .unwrap();
    assert_eq!(loaded.state.status, Status::Running);
    assert_eq!(loaded.next_action, 1);
}
#[cfg(unix)]
#[test]
fn dangling_lock_symlink_never_creates_external_target() {
    let (tmp, _, store) = init();
    let root = store.root().to_owned();
    drop(store);
    fs::remove_file(root.join("lock")).unwrap();
    let target = tmp.path().join("outside-target");
    std::os::unix::fs::symlink(&target, root.join("lock")).unwrap();
    assert!(RunStore::open(&root).is_err());
    assert!(!target.exists());
}
#[test]
fn read_only_status_works_with_exclusive_lock_and_does_not_create_one() {
    let (_tmp, record, store) = init();
    let root = store.root().to_owned();
    let before = fs::metadata(root.join("lock")).unwrap().modified().unwrap();
    let loaded = RunStore::read_status(&root).unwrap();
    assert_eq!(loaded.run_id, record.run_id);
    assert!(RunStore::open(&root).is_err());
    assert_eq!(
        fs::metadata(root.join("lock")).unwrap().modified().unwrap(),
        before
    );
    drop(store);
    fs::remove_file(root.join("lock")).unwrap();
    assert!(RunStore::read_status(&root).is_ok());
    assert!(!root.join("lock").exists());
}
#[test]
fn read_only_status_rejects_tampering_without_writing() {
    let (_tmp, _, store) = init();
    let root = store.root().to_owned();
    let path = root.join("snapshot.json");
    let original = fs::read(&path).unwrap();
    let mut snapshot: serde_json::Value = serde_json::from_slice(&original).unwrap();
    snapshot["template"] = "tampered".into();
    fs::write(&path, serde_json::to_vec(&snapshot).unwrap()).unwrap();
    let tampered = fs::read(&path).unwrap();
    assert!(RunStore::read_status(&root).is_err());
    assert_eq!(fs::read(&path).unwrap(), tampered);
}
#[cfg(unix)]
#[test]
fn read_only_status_rejects_record_symlinks_without_following_them() {
    let (tmp, _, store) = init();
    let root = store.root().to_owned();
    let outside = tmp.path().join("outside");
    fs::write(&outside, "keep").unwrap();
    fs::remove_file(root.join("state.json")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("state.json")).unwrap();
    assert!(RunStore::read_status(&root).is_err());
    assert_eq!(fs::read_to_string(outside).unwrap(), "keep");
}
