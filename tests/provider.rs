use routing::Gate;
use simon_says::provider::{extract_response, invoke, prepare, validate_native_args};
use simon_says::{routing, Role};
use std::{fs, path::Path};

fn role(kind: &str, adapter: Option<&str>) -> Role {
    Role {
        kind: kind.into(),
        argv: vec![],
        adapter: adapter.map(str::to_owned),
        model: None,
        effort: None,
        profile: None,
        resume_worker_session: false,
        timeout_seconds: None,
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
    let process: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("process.json")).unwrap()).unwrap();
    assert!(process["pid"].as_u64().unwrap() > 0);
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
fn native_terminal_error_cannot_be_accepted_on_zero_exit() {
    for (adapter, terminal) in [
        (
            "codex",
            r#"{"type":"turn.failed","error":{"message":"failed"}}"#,
        ),
        (
            "claude",
            r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"failed"}"#,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake.py");
        fs::write(&script, format!("print('{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"candidate\"}}}}')\nprint({terminal:?})\n")).unwrap();
        let mut role = role("command", None);
        role.argv = vec!["python3".into(), script.to_string_lossy().into()];
        let mut plan = prepare(
            &role,
            Gate::Worker,
            "",
            dir.path(),
            &dir.path().join("packet"),
        )
        .unwrap();
        plan.adapter = adapter.into();
        assert!(
            invoke(&plan, dir.path()).is_err(),
            "{adapter} terminal failure accepted"
        );
    }
}
#[test]
fn recoverable_native_warning_does_not_override_success() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"error\",\"message\":\"temporary warning\"}')\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"done\"}}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    let outcome = invoke(&plan, dir.path()).unwrap();
    assert!(outcome.success);
    assert_eq!(outcome.response.as_deref(), Some("done"));
}
#[test]
fn error_after_codex_response_requires_new_completed_response() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\nprint('{\"type\":\"error\",\"message\":\"failure\"}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn incomplete_native_stream_cannot_be_accepted_on_zero_exit() {
    for adapter in ["codex", "claude"] {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake.py");
        fs::write(&script, "print('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\n").unwrap();
        let mut role = role("command", None);
        role.argv = vec!["python3".into(), script.to_string_lossy().into()];
        let mut plan = prepare(
            &role,
            Gate::Worker,
            "",
            dir.path(),
            &dir.path().join("packet"),
        )
        .unwrap();
        plan.adapter = adapter.into();
        assert!(
            invoke(&plan, dir.path()).is_err(),
            "{adapter} incomplete stream accepted"
        );
    }
}
#[test]
fn native_success_cannot_precede_its_response() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"turn.completed\"}')\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn partial_codex_message_is_not_a_final_response() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"item.started\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn updated_message_after_terminal_invalidates_stream() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"turn.completed\"}')\nprint('{\"type\":\"item.updated\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn codex_uses_last_completed_message_before_success() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"earlier\"}}')\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"final\"}}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert_eq!(
        invoke(&plan, dir.path()).unwrap().response.as_deref(),
        Some("final")
    );
}
#[test]
fn large_native_event_stream_is_processed_without_accumulating_events() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "import sys\nsys.stdout.write('{\"type\":\"progress\"}\\n' * 100000)\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"done\"}}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert_eq!(
        invoke(&plan, dir.path()).unwrap().response.as_deref(),
        Some("done")
    );
}
#[test]
fn subsequent_unfinished_codex_turn_invalidates_prior_success() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\nprint('{\"type\":\"turn.completed\"}')\nprint('{\"type\":\"turn.started\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn later_native_work_or_session_start_invalidates_success() {
    for (adapter, first, later) in [
        (
            "codex",
            r#"{"type":"turn.completed"}"#,
            r#"{"type":"item.started","item":{"type":"command_execution","command":"work"}}"#,
        ),
        (
            "claude",
            r#"{"type":"result","subtype":"success","is_error":false,"result":"candidate"}"#,
            r#"{"type":"system","subtype":"init"}"#,
        ),
        (
            "codex",
            r#"{"type":"turn.completed"}"#,
            r#"{"type":"error","message":"late failure"}"#,
        ),
        (
            "claude",
            r#"{"type":"result","subtype":"success","is_error":false,"result":"candidate"}"#,
            r#"{"type":"error","message":"late failure"}"#,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake.py");
        fs::write(&script, format!("print({first:?})\nprint({later:?})\n")).unwrap();
        let mut role = role("command", None);
        role.argv = vec!["python3".into(), script.to_string_lossy().into()];
        let mut plan = prepare(
            &role,
            Gate::Worker,
            "",
            dir.path(),
            &dir.path().join("packet"),
        )
        .unwrap();
        plan.adapter = adapter.into();
        assert!(
            invoke(&plan, dir.path()).is_err(),
            "{adapter} accepted post-terminal work"
        );
    }
}
#[test]
fn trailing_truncated_native_event_invalidates_success() {
    for (adapter, first) in [
        ("codex", r#"{"type":"turn.completed"}"#),
        (
            "claude",
            r#"{"type":"result","subtype":"success","is_error":false,"result":"candidate"}"#,
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake.py");
        fs::write(&script, format!("print({first:?})\nprint('{{truncated')\n")).unwrap();
        let mut role = role("command", None);
        role.argv = vec!["python3".into(), script.to_string_lossy().into()];
        let mut plan = prepare(
            &role,
            Gate::Worker,
            "",
            dir.path(),
            &dir.path().join("packet"),
        )
        .unwrap();
        plan.adapter = adapter.into();
        assert!(
            invoke(&plan, dir.path()).is_err(),
            "{adapter} accepted truncated event"
        );
    }
}
#[test]
fn pretty_single_claude_success_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "import json\nprint(json.dumps({'type':'result','subtype':'success','is_error':False,'result':'done'}, indent=2))\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "claude".into();
    assert_eq!(
        invoke(&plan, dir.path()).unwrap().response.as_deref(),
        Some("done")
    );
}
#[test]
fn later_codex_turn_cannot_reuse_an_earlier_response() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"old candidate\"}}')\nprint('{\"type\":\"turn.completed\"}')\nprint('{\"type\":\"turn.started\"}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn codex_rejects_work_after_terminal_even_with_another_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"turn.started\"}')\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\nprint('{\"type\":\"turn.completed\"}')\nprint('{\"type\":\"item.started\",\"item\":{\"type\":\"command_execution\"}}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn codex_rejects_overlapping_turns() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"turn.started\"}')\nprint('{\"type\":\"item.started\",\"item\":{\"type\":\"command_execution\",\"id\":\"one\"}}')\nprint('{\"type\":\"turn.started\"}')\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn codex_requires_started_work_to_complete_and_allows_concurrent_items() {
    for complete in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake.py");
        let finishing = if complete {
            "print('{\"type\":\"item.completed\",\"item\":{\"type\":\"command_execution\",\"id\":\"one\"}}')\n"
        } else {
            ""
        };
        fs::write(&script, format!("print('{{\"type\":\"turn.started\"}}')\nprint('{{\"type\":\"item.started\",\"item\":{{\"type\":\"command_execution\",\"id\":\"one\"}}}}')\nprint('{{\"type\":\"item.started\",\"item\":{{\"type\":\"command_execution\",\"id\":\"two\"}}}}')\nprint('{{\"type\":\"item.updated\",\"item\":{{\"type\":\"command_execution\",\"id\":\"one\"}}}}')\nprint('{{\"type\":\"item.completed\",\"item\":{{\"type\":\"command_execution\",\"id\":\"two\"}}}}')\n{finishing}print('{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"candidate\"}}}}')\nprint('{{\"type\":\"turn.completed\"}}')\n")).unwrap();
        let mut role = role("command", None);
        role.argv = vec!["python3".into(), script.to_string_lossy().into()];
        let mut plan = prepare(
            &role,
            Gate::Worker,
            "",
            dir.path(),
            &dir.path().join("packet"),
        )
        .unwrap();
        plan.adapter = "codex".into();
        assert_eq!(invoke(&plan, dir.path()).is_ok(), complete);
    }
}
#[test]
fn codex_rejects_an_update_for_untracked_work() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"turn.started\"}')\nprint('{\"type\":\"item.updated\",\"item\":{\"type\":\"command_execution\",\"id\":\"missing\"}}')\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"candidate\"}}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert!(invoke(&plan, dir.path()).is_err());
}
#[test]
fn codex_rejects_excess_outstanding_work_and_invalid_ids() {
    for program in [
        "for i in range(4097): print(json.dumps({'type':'item.started','item':{'id':str(i)}}))",
        "print(json.dumps({'type':'item.started','item':{'id':'x'*513}}))",
        "print(json.dumps({'type':'item.started','item':{'id':'same'}})); print(json.dumps({'type':'item.started','item':{'id':'same'}}))",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake.py");
        fs::write(&script, format!("import json\n{program}\nprint('{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"candidate\"}}}}')\nprint('{{\"type\":\"turn.completed\"}}')\n")).unwrap();
        let mut role = role("command", None);
        role.argv = vec!["python3".into(), script.to_string_lossy().into()];
        let mut plan = prepare(&role, Gate::Worker, "", dir.path(), &dir.path().join("packet")).unwrap();
        plan.adapter = "codex".into();
        assert!(invoke(&plan, dir.path()).is_err());
    }
}
#[test]
fn codex_accepts_multiple_complete_turns_and_uses_latest_response() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"turn.started\"}')\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"old\"}}')\nprint('{\"type\":\"turn.completed\"}')\nprint('{\"type\":\"turn.started\"}')\nprint('{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"new\"}}')\nprint('{\"type\":\"turn.completed\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "codex".into();
    assert_eq!(
        invoke(&plan, dir.path()).unwrap().response.as_deref(),
        Some("new")
    );
}
#[test]
fn claude_success_result_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"error\",\"message\":\"temporary warning\"}')\nprint('{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"done\"}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "claude".into();
    let outcome = invoke(&plan, dir.path()).unwrap();
    assert!(outcome.success);
    assert_eq!(outcome.response.as_deref(), Some("done"));
}
#[test]
fn claude_result_requires_explicit_success_and_no_error() {
    for event in [
        r#"{"type":"result","result":"candidate"}"#,
        r#"{"type":"result","subtype":"cancelled","is_error":false,"result":"candidate"}"#,
        r#"{"type":"result","subtype":"failed","is_error":false,"result":"candidate"}"#,
        r#"{"type":"result","subtype":"success","result":"candidate"}"#,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake.py");
        fs::write(&script, format!("print({event:?})\n")).unwrap();
        let mut role = role("command", None);
        role.argv = vec!["python3".into(), script.to_string_lossy().into()];
        let mut plan = prepare(
            &role,
            Gate::Worker,
            "",
            dir.path(),
            &dir.path().join("packet"),
        )
        .unwrap();
        plan.adapter = "claude".into();
        assert!(invoke(&plan, dir.path()).is_err(), "accepted {event}");
    }
}
#[test]
fn later_claude_cancelled_result_overrides_prior_success_without_text() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake.py");
    fs::write(&script, "print('{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"result\":\"candidate\"}')\nprint('{\"type\":\"result\",\"subtype\":\"cancelled\",\"is_error\":false}')\n").unwrap();
    let mut role = role("command", None);
    role.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let mut plan = prepare(
        &role,
        Gate::Worker,
        "",
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    plan.adapter = "claude".into();
    assert!(invoke(&plan, dir.path()).is_err());
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
    assert_eq!(p.argv.last().unwrap(), "-");
    assert_eq!(p.stdin_prompt.as_deref(), Some("hello"));
    r.adapter = Some("claude".into());
    let p = prepare(&r, Gate::Worker, "hello", dir.path(), &packet).unwrap();
    assert_eq!(p.program, "claude");
    assert!(p.argv.windows(2).any(|v| v == ["--effort", "high"]));
    assert!(p
        .argv
        .windows(2)
        .any(|v| v == ["--permission-mode", "acceptEdits"]));
    assert_eq!(p.argv.last().unwrap(), "--");
    assert_eq!(p.stdin_prompt.as_deref(), Some("hello"));
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
        "--last",
        "--all",
        "--worktree",
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
#[cfg(unix)]
#[test]
fn surviving_action_process_group_is_uncertain() {
    let tmp = tempfile::tempdir().unwrap();
    let packet = tmp.path().join("packet.json");
    fs::write(&packet, "{}").unwrap();
    let script = tmp.path().join("spawn.py");
    fs::write(&script, "import subprocess\nsubprocess.Popen(['python3','-c','import time; time.sleep(5)'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)\nprint('{}')\n").unwrap();
    let mut r = role("command", None);
    r.argv = vec!["python3".into(), script.to_string_lossy().into()];
    let plan = prepare(&r, Gate::Worker, "", tmp.path(), &packet).unwrap();
    let actions = tmp.path().join("action");
    fs::create_dir(&actions).unwrap();
    let error = invoke(&plan, &actions).unwrap_err();
    assert!(error.to_string().contains("processes running"));
}
#[test]
fn timeout_stops_action_without_claiming_success() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = role("command", None);
    r.argv = vec!["sh".into(), "-c".into(), "sleep 30".into()];
    r.timeout_seconds = Some(1);
    let plan = prepare(&r, Gate::Worker, "", dir.path(), &dir.path().join("packet")).unwrap();
    let started = std::time::Instant::now();
    let error = invoke(&plan, dir.path()).unwrap_err();
    assert!(error.to_string().contains("timed out"));
    assert!(error.to_string().contains("termination attempted"));
    assert!(error.to_string().contains("leader reaped: true"));
    assert!(error
        .to_string()
        .contains("external effects remain uncertain"));
    assert!(started.elapsed() < std::time::Duration::from_secs(8));
    assert!(dir.path().join("process.json").exists());
}
#[test]
fn oversized_action_log_stops_the_owned_process_group() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = role("command", None);
    r.argv = vec![
        "python3".into(),
        "-c".into(),
        "import os,time; os.ftruncate(1, 64*1024*1024+1); time.sleep(30)".into(),
    ];
    let plan = prepare(&r, Gate::Worker, "", dir.path(), &dir.path().join("packet")).unwrap();
    let started = std::time::Instant::now();
    let error = invoke(&plan, dir.path()).unwrap_err();
    assert!(error.to_string().contains("64 MiB"));
    assert!(started.elapsed() < std::time::Duration::from_secs(8));
    assert!(fs::metadata(dir.path().join("stdout.txt")).unwrap().len() > 64 * 1024 * 1024);
}
#[test]
fn failed_process_receipt_does_not_leave_child_running() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("child-survived");
    fs::write(dir.path().join("process.json"), "occupied").unwrap();
    let mut r = role("command", None);
    r.argv = vec![
        "sh".into(),
        "-c".into(),
        format!("sleep 1; touch '{}'", marker.display()),
    ];
    let plan = prepare(&r, Gate::Worker, "", dir.path(), &dir.path().join("packet")).unwrap();
    let error = invoke(&plan, dir.path()).unwrap_err();
    assert!(error.to_string().contains("record action process"));
    std::thread::sleep(std::time::Duration::from_millis(1300));
    assert!(
        !marker.exists(),
        "child must not continue after receipt failure"
    );
}
#[test]
fn native_provider_uses_stdin_for_large_prompt_and_bounds_input() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = role("agent", Some("codex"));
    r.profile = Some("profile.md".into());
    let prompt = "x".repeat(256 * 1024);
    let plan = prepare(
        &r,
        Gate::Reviewer,
        &prompt,
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap();
    assert_eq!(plan.argv.last().unwrap(), "-");
    assert_eq!(plan.stdin_prompt.as_deref(), Some(prompt.as_str()));
    let serialized = serde_json::to_value(&plan).unwrap();
    assert_eq!(serialized["stdin_prompt"], prompt);
    let error = prepare(
        &r,
        Gate::Reviewer,
        &"x".repeat(4 * 1024 * 1024 + 1),
        dir.path(),
        &dir.path().join("packet"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("4 MiB"));
}
#[test]
fn file_backed_stdin_delivers_exact_prompt_without_pipe_deadlock() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = role("command", None);
    r.argv = vec!["sh".into(), "-c".into(), "cat".into()];
    let mut plan = prepare(&r, Gate::Worker, "", dir.path(), &dir.path().join("packet")).unwrap();
    let prompt = "x".repeat(256 * 1024);
    plan.stdin_prompt = Some(prompt.clone());
    let out = invoke(&plan, dir.path()).unwrap();
    assert!(out.success);
    assert_eq!(out.response.as_deref(), Some(prompt.as_str()));
    assert_eq!(
        fs::read_to_string(dir.path().join("prompt.txt")).unwrap(),
        prompt
    );
}
#[test]
fn file_backed_prompt_still_obeys_timeout_if_child_never_reads() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = role("command", None);
    r.argv = vec!["sh".into(), "-c".into(), "sleep 30".into()];
    r.timeout_seconds = Some(1);
    let mut plan = prepare(&r, Gate::Worker, "", dir.path(), &dir.path().join("packet")).unwrap();
    plan.stdin_prompt = Some("x".repeat(1024 * 1024));
    let start = std::time::Instant::now();
    let err = invoke(&plan, dir.path()).unwrap_err();
    assert!(err.to_string().contains("timed out"));
    assert!(start.elapsed() < std::time::Duration::from_secs(8));
    assert_eq!(
        fs::metadata(dir.path().join("prompt.txt")).unwrap().len(),
        1024 * 1024
    );
}
#[test]
fn claude_workers_require_nested_sandbox_probe_without_guessing_policy() {
    use simon_says::provider::requires_nested_sandbox_probe;
    let mut claude = role("agent", Some("claude"));
    assert!(requires_nested_sandbox_probe(&claude, Gate::Worker));
    claude.args = vec!["--permission-mode".into(), "acceptEdits".into()];
    assert!(requires_nested_sandbox_probe(&claude, Gate::Worker));
    assert!(!requires_nested_sandbox_probe(&claude, Gate::Reviewer));
    assert!(!requires_nested_sandbox_probe(&claude, Gate::Unblocker));
    assert!(!requires_nested_sandbox_probe(
        &role("agent", Some("codex")),
        Gate::Worker
    ));
    assert!(!requires_nested_sandbox_probe(
        &role("command", None),
        Gate::Worker
    ));
}

#[test]
fn codex_session_continuation_is_explicit_worker_only_and_identity_bound() {
    use simon_says::provider::prepare_with_session;
    let dir = tempfile::tempdir().unwrap();
    let packet = dir.path().join("packet");
    let id = "01a198db-e635-7d30-bd63-17dabefeb094";
    let mut r = role("agent", Some("codex"));
    r.model = Some("model".into());
    r.effort = Some("high".into());
    assert!(
        prepare_with_session(&r, Gate::Worker, "prompt", dir.path(), &packet, Some(id)).is_err()
    );
    r.resume_worker_session = true;
    let p =
        prepare_with_session(&r, Gate::Worker, "prompt", dir.path(), &packet, Some(id)).unwrap();
    assert!(p.argv.windows(2).any(|a| a == ["resume", id]));
    assert!(p.argv.windows(2).any(|a| a == ["--model", "model"]));
    assert_eq!(p.argv.last().unwrap(), "-");
    assert_eq!(p.session_in.as_deref(), Some(id));
    assert_eq!(p.stdin_prompt.as_deref(), Some("prompt"));
    assert!(prepare_with_session(&r, Gate::Reviewer, "", dir.path(), &packet, Some(id)).is_err());
    assert!(prepare_with_session(&r, Gate::Unblocker, "", dir.path(), &packet, Some(id)).is_err());
    for invalid in [
        "--last",
        "not-a-uuid",
        "01A198DB-E635-7D30-BD63-17DABEFEB094",
        "01a198db-e635-7d30-bd63-17dabefeb094\n",
    ] {
        assert!(
            prepare_with_session(&r, Gate::Worker, "", dir.path(), &packet, Some(invalid)).is_err(),
            "{invalid:?}"
        );
    }
    r.args.push("--ephemeral=true".into());
    assert!(prepare(&r, Gate::Worker, "", dir.path(), &packet).is_err());
    r.args.clear();
    r.adapter = Some("claude".into());
    assert!(prepare(&r, Gate::Worker, "", dir.path(), &packet).is_err());
}
