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
