use simon_says::{routing, Role};
#[path = "../src/provider.rs"]
mod provider;
use provider::{extract_response, invoke, prepare, validate_native_args};
use routing::Gate;
use std::{fs, path::Path};

fn role(kind: &str, adapter: Option<&str>) -> Role {
    Role {
        kind: kind.into(),
        argv: vec![],
        adapter: adapter.map(str::to_owned),
        model: None,
        effort: None,
        profile: None,
        args: vec![],
    }
}
#[test]
fn command_receives_packet_and_output_is_logged() {
    let dir = tempfile::tempdir().unwrap();
    let packet = dir.path().join("packet.json");
    fs::write(&packet, "packet contents").unwrap();
    let mut r = role("command", None);
    r.argv = vec![
        "sh".into(),
        "-c".into(),
        "cat \"$SIMON_PACKET\"; echo diag >&2".into(),
    ];
    let plan = prepare(&r, Gate::Worker, "unused", dir.path(), &packet).unwrap();
    let out = invoke(&plan, dir.path()).unwrap();
    assert!(out.success);
    assert_eq!(out.response.as_deref(), Some("packet contents"));
    assert_eq!(
        fs::read_to_string(dir.path().join("stderr.txt")).unwrap(),
        "diag\n"
    );
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn command_failure_is_returned_without_guessing_success() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = role("command", None);
    r.argv = vec!["sh".into(), "-c".into(), "echo broken >&2; exit 17".into()];
    let p = prepare(&r, Gate::Worker, "", dir.path(), &dir.path().join("packet")).unwrap();
    let out = invoke(&p, dir.path()).unwrap();
    assert!(!out.success);
    assert_eq!(out.exit_code, Some(17));
}
#[test]
fn codex_and_claude_plans_include_exact_model_and_effort() {
    let dir = tempfile::tempdir().unwrap();
    let packet = dir.path().join("packet");
    let mut r = role("agent", Some("codex"));
    r.model = Some("model".into());
    r.effort = Some("high".into());
    let p = prepare(&r, Gate::Reviewer, "hello", dir.path(), &packet).unwrap();
    assert_eq!(p.program, "codex");
    assert!(p.argv.windows(2).any(|v| v == ["--model", "model"]));
    assert!(p
        .argv
        .contains(&"model_reasoning_effort=\"high\"".to_string()));
    assert_eq!(p.argv.last().unwrap(), "hello");
    r.adapter = Some("claude".into());
    let p = prepare(&r, Gate::Worker, "hello", dir.path(), &packet).unwrap();
    assert_eq!(p.program, "claude");
    assert!(p.argv.windows(2).any(|v| v == ["--effort", "high"]));
    assert!(p
        .argv
        .windows(2)
        .any(|v| v == ["--permission-mode", "acceptEdits"]));
    assert_eq!(p.argv[p.argv.len() - 2], "--");
    let review = prepare(&r, Gate::Reviewer, "hello", dir.path(), &packet).unwrap();
    assert!(!review.argv.iter().any(|a| a == "--permission-mode"));
}
#[test]
fn native_owned_flags_and_bad_effort_are_rejected() {
    for arg in [
        "--model=evil",
        "--cd=/tmp",
        "-C/tmp",
        "-cmodel_reasoning_effort=low",
        "--json",
        "resume",
    ] {
        assert!(
            validate_native_args("codex", &[arg.into()]).is_err(),
            "{arg}"
        );
    }
    for arg in ["--output-format=json", "--effort=high", "--resume", "--"] {
        assert!(
            validate_native_args("claude", &[arg.into()]).is_err(),
            "{arg}"
        );
    }
    let mut r = role("agent", Some("codex"));
    r.effort = Some("imaginary".into());
    assert!(prepare(&r, Gate::Worker, "", Path::new("."), Path::new("/tmp/p")).is_err());
}
#[test]
fn extracts_native_final_messages_and_session_ids() {
    let cases = [
        (
            r#"{"type":"thread.started","thread_id":"a"}
{"type":"item.completed","item":{"type":"agent_message","text":"{\"ok\":true}"}}"#,
            "a",
            "{\"ok\":true}",
        ),
        (
            r#"{"type":"result","session_id":"b","result":"done"}"#,
            "b",
            "done",
        ),
    ];
    for (input, id, response) in cases {
        let (r, s) = extract_response(input);
        assert_eq!(r.as_deref(), Some(response));
        assert_eq!(s.as_deref(), Some(id));
    }
    assert_eq!(extract_response("unstructured text"), (None, None));
}
