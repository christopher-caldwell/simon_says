use simon_says::{
    routing::{Gate, Response, State, Status},
    Workflow,
};
use std::{fs, path::Path};
fn setup() -> (Workflow, State) {
    let t = fs::read_to_string("examples/basic/workflow.toml").unwrap();
    let (w, _) = Workflow::parse_and_validate(&t, Path::new("examples/basic")).unwrap();
    let s = State::new(&w, "baseline".into());
    (w, s)
}
fn act(
    s: &mut State,
    w: &Workflow,
    outcome: &str,
    candidate: Option<&str>,
    inspected: Option<&str>,
) {
    s.start("id".into()).unwrap();
    s.apply(
        w,
        Response {
            action_id: "id".into(),
            outcome: outcome.into(),
            report: "full feedback".into(),
            candidate_commit: candidate.map(str::to_string),
            inspected_commit: inspected.map(str::to_string),
        },
    )
    .unwrap();
}
#[test]
fn pass_and_correction_route_to_same_stage_and_preserve_feedback() {
    let (w, mut s) = setup();
    act(&mut s, &w, "complete", Some("c1"), None);
    act(&mut s, &w, "changes_required", None, Some("c1"));
    assert_eq!(s.gate, Gate::Worker);
    assert_eq!(s.stage, "validate");
    assert_eq!(s.accepted_commit, "baseline");
    assert_eq!(s.candidate_commit.as_deref(), Some("c1"));
    assert_eq!(s.feedback.len(), 2);
    act(&mut s, &w, "complete", Some("c2"), None);
    act(&mut s, &w, "pass", None, Some("c2"));
    assert_eq!(s.stage, "report");
    assert_eq!(s.accepted_commit, "c2");
    act(&mut s, &w, "complete", Some("c3"), None);
    act(&mut s, &w, "pass", None, Some("c3"));
    assert_eq!(s.status, Status::Complete);
}
#[test]
fn bounded_unblock_retries_origin_and_stops_on_second_block() {
    let (w, mut s) = setup();
    act(&mut s, &w, "blocked", None, None);
    assert_eq!(s.gate, Gate::Unblocker);
    assert_eq!(s.unblock_origin, Some(Gate::Worker));
    act(&mut s, &w, "retry", None, None);
    assert_eq!(s.gate, Gate::Worker);
    act(&mut s, &w, "blocked", None, None);
    assert_eq!(s.status, Status::Stopped);
}
#[test]
fn reviewer_block_retries_same_candidate() {
    let (w, mut s) = setup();
    act(&mut s, &w, "complete", Some("candidate"), None);
    act(&mut s, &w, "blocked", None, Some("candidate"));
    act(&mut s, &w, "retry", None, None);
    assert_eq!(s.gate, Gate::Reviewer);
    assert_eq!(s.candidate_commit.as_deref(), Some("candidate"));
}
#[test]
fn invalid_identity_and_wrong_inspected_commit_cannot_advance() {
    let (w, mut s) = setup();
    s.start("expected".into()).unwrap();
    let r = Response {
        action_id: "wrong".into(),
        outcome: "complete".into(),
        report: "x".into(),
        candidate_commit: Some("c".into()),
        inspected_commit: None,
    };
    assert!(s.apply(&w, r).is_err());
    assert_eq!(s.status, Status::Running);
    s.apply(
        &w,
        Response {
            action_id: "expected".into(),
            outcome: "complete".into(),
            report: "x".into(),
            candidate_commit: Some("c".into()),
            inspected_commit: None,
        },
    )
    .unwrap();
    s.start("review".into()).unwrap();
    assert!(s
        .apply(
            &w,
            Response {
                action_id: "review".into(),
                outcome: "pass".into(),
                report: "x".into(),
                candidate_commit: None,
                inspected_commit: Some("different".into())
            }
        )
        .is_err());
    assert_eq!(s.accepted_commit, "baseline");
}
#[test]
fn correction_limit_stops() {
    let (w, mut s) = setup();
    for n in 0..3 {
        let c = format!("c{n}");
        act(&mut s, &w, "complete", Some(&c), None);
        act(&mut s, &w, "changes_required", None, Some(&c));
    }
    assert_eq!(s.status, Status::Stopped);
    assert_eq!(s.review_cycles, 3);
}
