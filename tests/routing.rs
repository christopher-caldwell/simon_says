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
            route: None,
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
        route: None,
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
            route: None,
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
                inspected_commit: Some("different".into()),
                route: None,
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

fn dynamic() -> (Workflow, State) {
    let t = fs::read_to_string("examples/basic/workflow.toml")
        .unwrap()
        .replace(
            "start = \"validate\"",
            "start = \"validate\"\nmax_stage_transitions = 3",
        )
        .replace(
            "on_pass = \"report\"",
            "on_pass = \"report\"\nroutes = { issues = \"report\", clean = \"complete\" }",
        )
        .replace("on_pass = \"complete\"", "on_pass = \"validate\"");
    let (w, _) = Workflow::parse_and_validate(&t, Path::new("examples/basic")).unwrap();
    let s = State::new(&w, "baseline".into());
    (w, s)
}

fn review_route(
    s: &mut State,
    w: &Workflow,
    candidate: &str,
    route: Option<&str>,
) -> anyhow::Result<()> {
    s.start("review".into())?;
    s.apply(
        w,
        Response {
            action_id: "review".into(),
            outcome: "pass".into(),
            report: "reviewed".into(),
            candidate_commit: None,
            inspected_commit: Some(candidate.into()),
            route: route.map(str::to_owned),
        },
    )
}

#[test]
fn dynamic_routes_require_exact_reviewed_candidate_and_declared_label() {
    let (w, mut s) = dynamic();
    act(&mut s, &w, "complete", Some("issue-commit"), None);
    assert!(review_route(&mut s, &w, "issue-commit", None).is_err());
    assert_eq!(s.status, Status::Running);
    for route in ["unknown", "../report", "report"] {
        assert!(s
            .apply(
                &w,
                Response {
                    action_id: "review".into(),
                    outcome: "pass".into(),
                    report: "reviewed".into(),
                    candidate_commit: None,
                    inspected_commit: Some("issue-commit".into()),
                    route: Some(route.into())
                }
            )
            .is_err());
    }
    assert_eq!(s.accepted_commit, "baseline");
    s.apply(
        &w,
        Response {
            action_id: "review".into(),
            outcome: "pass".into(),
            report: "reviewed".into(),
            candidate_commit: None,
            inspected_commit: Some("issue-commit".into()),
            route: Some("issues".into()),
        },
    )
    .unwrap();
    assert_eq!(s.stage, "report");
    assert_eq!(s.stage_transitions, 1);
    let prev = s.last_transition.unwrap();
    assert_eq!(prev.reviewed_commit, "issue-commit");
    assert_eq!(prev.selected_route.as_deref(), Some("issues"));
    assert_eq!(prev.from_stage, "validate");
    assert_eq!(prev.to_stage, "report");
}

#[test]
fn cycle_budget_survives_serialization_and_stops_without_another_transition() {
    let (w, mut s) = dynamic();
    act(&mut s, &w, "complete", Some("c1"), None);
    review_route(&mut s, &w, "c1", Some("issues")).unwrap();
    act(&mut s, &w, "complete", Some("c2"), None);
    act(&mut s, &w, "pass", None, Some("c2"));
    s = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    act(&mut s, &w, "complete", Some("c3"), None);
    review_route(&mut s, &w, "c3", Some("issues")).unwrap();
    assert_eq!(s.status, Status::Stopped);
    assert_eq!(
        s.stop_reason.as_deref(),
        Some("workflow stage transition limit reached")
    );
    assert_eq!(s.stage_transitions, 2);
    assert_eq!(s.accepted_commit, "c3");
    assert_eq!(s.stage, "validate");
    assert!(s.start("extra".into()).is_err());
}

#[test]
fn last_transition_can_complete_but_cannot_dispatch_another_stage() {
    let (mut w, mut s) = dynamic();
    w.max_stage_transitions = Some(1);
    act(&mut s, &w, "complete", Some("clean"), None);
    review_route(&mut s, &w, "clean", Some("clean")).unwrap();
    assert_eq!(s.status, Status::Complete);
    assert_eq!(s.stage_transitions, 1);
    let mut s = State::new(&w, "baseline".into());
    act(&mut s, &w, "complete", Some("issues"), None);
    review_route(&mut s, &w, "issues", Some("issues")).unwrap();
    assert_eq!(s.status, Status::Stopped);
    assert_eq!(s.stage_transitions, 0);
    assert_eq!(s.stage, "validate");
    let mut s = State::new(&w, "baseline".into());
    w.stages.get_mut("validate").unwrap().approval_required = true;
    act(&mut s, &w, "complete", Some("issues-approval"), None);
    review_route(&mut s, &w, "issues-approval", Some("issues")).unwrap();
    assert_eq!(s.status, Status::Stopped);
    assert!(s.pending_approval.is_none());
    assert_eq!(s.action_id, None);
    assert_eq!(s.accepted_commit, "issues-approval");
    assert_eq!(s.stage, "validate");
    assert_eq!(s.stage_transitions, 0);
    assert!(s.start("extra".into()).is_err());

    let mut s = State::new(&w, "baseline".into());
    act(&mut s, &w, "complete", Some("clean-approval"), None);
    review_route(&mut s, &w, "clean-approval", Some("clean")).unwrap();
    assert_eq!(s.status, Status::AwaitingApproval);
    let id = s.pending_approval.as_ref().unwrap().approval_id.clone();
    s.status = Status::ApprovalChecking;
    s.approve(&w, "clean-approval", "digest", &id, 3, None)
        .unwrap();
    assert_eq!(s.status, Status::Complete);
    assert_eq!(s.stage_transitions, 1);
}

#[test]
fn approval_preserves_and_dispatches_reviewed_route_after_restart() {
    let (mut w, mut s) = dynamic();
    w.stages.get_mut("validate").unwrap().approval_required = true;
    act(&mut s, &w, "complete", Some("c1"), None);
    review_route(&mut s, &w, "c1", Some("issues")).unwrap();
    assert_eq!(s.status, Status::AwaitingApproval);
    let approval_id = s.pending_approval.as_ref().unwrap().approval_id.clone();
    s = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    s.status = Status::ApprovalChecking;
    s.approve(&w, "c1", "digest", &approval_id, 1, None)
        .unwrap();
    assert_eq!(s.stage, "report");
    assert_eq!(
        s.last_transition.unwrap().selected_route.as_deref(),
        Some("issues")
    );
}

#[test]
fn worker_and_nonpass_reviewer_cannot_select_route() {
    let (w, mut s) = dynamic();
    s.start("id".into()).unwrap();
    assert!(s
        .apply(
            &w,
            Response {
                action_id: "id".into(),
                outcome: "complete".into(),
                report: "x".into(),
                candidate_commit: Some("c1".into()),
                inspected_commit: None,
                route: Some("clean".into())
            }
        )
        .is_err());
    assert_eq!(s.candidate_commit, None);
    s.apply(
        &w,
        Response {
            action_id: "id".into(),
            outcome: "complete".into(),
            report: "x".into(),
            candidate_commit: Some("c1".into()),
            inspected_commit: None,
            route: None,
        },
    )
    .unwrap();
    s.start("review".into()).unwrap();
    assert!(s
        .apply(
            &w,
            Response {
                action_id: "review".into(),
                outcome: "changes_required".into(),
                report: "x".into(),
                candidate_commit: None,
                inspected_commit: Some("c1".into()),
                route: Some("clean".into())
            }
        )
        .is_err());
}
