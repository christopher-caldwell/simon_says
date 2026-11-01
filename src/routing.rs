//! Pure transition logic; execution and checkpoint checks are in the controller.
use crate::Workflow;
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Gate {
    Worker,
    Reviewer,
    Unblocker,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Initializing,
    Ready,
    Preparing,
    Running,
    Stopped,
    AwaitingApproval,
    ApprovalChecking,
    Complete,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub action_id: String,
    pub outcome: String,
    pub report: String,
    pub candidate_commit: Option<String>,
    pub inspected_commit: Option<String>,
    /// A declared route label, selected only by a passing Reviewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub approval_id: String,
    pub review_action: String,
    pub stage: String,
    pub commit: String,
    pub snapshot_digest: String,
    pub approved_at_unix: u64,
    pub note: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingApproval {
    pub approval_id: String,
    pub stage: String,
    pub review_action: String,
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub from_stage: String,
    pub to_stage: String,
    pub reviewed_commit: String,
    pub review_action: String,
    pub selected_route: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub stage: String,
    pub gate: Gate,
    pub status: Status,
    pub action_id: Option<String>,
    pub accepted_commit: String,
    pub candidate_commit: Option<String>,
    pub feedback: Vec<String>,
    pub review_cycles: u32,
    pub unblock_origin: Option<Gate>,
    pub unblock_used: bool,
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub approvals: Vec<Approval>,
    #[serde(default)]
    pub pending_approval: Option<PendingApproval>,
    /// Cumulative accepted transitions; never reset during correction or reset.
    #[serde(default)]
    pub stage_transitions: u32,
    #[serde(default)]
    pub last_transition: Option<Transition>,
}
impl State {
    pub fn new(workflow: &Workflow, baseline: String) -> Self {
        Self {
            stage: workflow.start.clone(),
            gate: Gate::Worker,
            status: Status::Ready,
            action_id: None,
            accepted_commit: baseline,
            candidate_commit: None,
            feedback: vec![],
            review_cycles: 0,
            unblock_origin: None,
            unblock_used: false,
            stop_reason: None,
            approvals: Vec::new(),
            pending_approval: None,
            stage_transitions: 0,
            last_transition: None,
        }
    }
    pub fn start(&mut self, action_id: String) -> Result<()> {
        ensure!(
            self.status == Status::Ready
                || (self.status == Status::Preparing
                    && self.action_id.as_deref() == Some(action_id.as_str())),
            "action can only start from ready or matching preparation"
        );
        ensure!(!action_id.trim().is_empty(), "empty action identity");
        self.action_id = Some(action_id);
        self.status = Status::Running;
        Ok(())
    }
    /// The caller MUST verify Git and output mechanics before applying a complete or pass result.
    /// This pure function does not verify commits, artifacts, persistence or action isolation.
    pub fn apply(&mut self, workflow: &Workflow, response: Response) -> Result<()> {
        ensure!(
            self.status == Status::Running,
            "response requires running action"
        );
        ensure!(
            self.action_id.as_deref() == Some(response.action_id.as_str()),
            "action identity mismatch"
        );
        ensure!(!response.report.trim().is_empty(), "empty action report");
        let stage = workflow
            .stages
            .get(&self.stage)
            .ok_or_else(|| anyhow::anyhow!("unknown current stage"))?;
        let destination = if self.gate == Gate::Reviewer && response.outcome == "pass" {
            if stage.routes.is_empty() {
                ensure!(
                    response.route.is_none(),
                    "route is not declared for this stage"
                );
                Some(stage.on_pass.clone())
            } else {
                let route = response
                    .route
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("reviewer pass requires a declared route"))?;
                Some(
                    stage
                        .routes
                        .get(route)
                        .ok_or_else(|| anyhow::anyhow!("unknown reviewer route {route:?}"))?
                        .clone(),
                )
            }
        } else {
            ensure!(
                response.route.is_none(),
                "only a passing reviewer may select a route"
            );
            None
        };
        match (self.gate, response.outcome.as_str()) {
            (Gate::Worker, "complete") => {
                let commit = response
                    .candidate_commit
                    .as_deref()
                    .filter(|c| !c.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("worker complete requires candidate commit"))?;
                ensure!(
                    response.inspected_commit.is_none(),
                    "worker cannot report inspected commit"
                );
                self.candidate_commit = Some(commit.to_owned());
                self.feedback.push(response.report);
                self.gate = Gate::Reviewer;
                self.unblock_used = false;
            }
            (Gate::Reviewer, "pass" | "changes_required" | "blocked") => {
                ensure!(
                    response.candidate_commit.is_none(),
                    "reviewer cannot nominate candidate"
                );
                ensure!(
                    response.inspected_commit.as_deref() == self.candidate_commit.as_deref()
                        && self.candidate_commit.is_some(),
                    "review did not inspect candidate"
                );
                self.feedback.push(response.report);
                match response.outcome.as_str() {
                    "pass" => {
                        self.accepted_commit = self.candidate_commit.take().unwrap();
                        if stage.approval_required {
                            self.pending_approval = Some(PendingApproval {
                                approval_id: uuid::Uuid::new_v4().to_string(),
                                stage: self.stage.clone(),
                                review_action: response.action_id.clone(),
                                commit: self.accepted_commit.clone(),
                                route: response.route.clone(),
                            });
                            self.status = Status::AwaitingApproval;
                        } else {
                            self.advance(
                                workflow,
                                destination.as_deref().unwrap(),
                                &response.action_id,
                                response.route.as_deref(),
                            );
                        }
                    }
                    "changes_required" => {
                        self.review_cycles += 1;
                        if self.review_cycles >= stage.max_review_cycles {
                            self.stop_reason = Some("review cycle limit reached".into());
                            self.status = Status::Stopped;
                        } else {
                            self.gate = Gate::Worker;
                            self.unblock_used = false;
                        }
                    }
                    _ => self.block(),
                }
            }
            (Gate::Worker, "blocked") => {
                ensure!(
                    response.candidate_commit.is_none() && response.inspected_commit.is_none(),
                    "blocked worker cannot report commits"
                );
                self.feedback.push(response.report);
                self.block();
            }
            (Gate::Unblocker, "retry" | "blocked") => {
                ensure!(
                    response.candidate_commit.is_none() && response.inspected_commit.is_none(),
                    "unblocker cannot report commits"
                );
                let origin = self
                    .unblock_origin
                    .ok_or_else(|| anyhow::anyhow!("missing unblock origin"))?;
                self.feedback.push(response.report);
                if response.outcome == "retry" {
                    self.gate = origin;
                    self.unblock_origin = None;
                } else {
                    self.status = Status::Stopped;
                    self.stop_reason = Some("unblocker blocked".into());
                }
            }
            _ => anyhow::bail!("invalid outcome for gate"),
        }
        self.action_id = None;
        if self.status == Status::Running {
            self.status = Status::Ready;
        }
        Ok(())
    }
    /// Record an explicit approval of the exact reviewed checkpoint and continue the frozen route.
    pub fn approve(
        &mut self,
        workflow: &Workflow,
        commit: &str,
        digest: &str,
        approval_id: &str,
        approved_at_unix: u64,
        note: Option<String>,
    ) -> Result<()> {
        ensure!(
            self.status == Status::ApprovalChecking,
            "approval is not being checked"
        );
        let pending = self
            .pending_approval
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("pending approval identity missing"))?;
        ensure!(
            pending.approval_id == approval_id
                && pending.stage == self.stage
                && pending.commit == commit,
            "approval identity does not match pending review"
        );
        ensure!(
            self.accepted_commit == commit,
            "approval commit does not match accepted checkpoint"
        );
        ensure!(!digest.is_empty(), "approval requires workflow identity");
        let stage = workflow
            .stages
            .get(&self.stage)
            .ok_or_else(|| anyhow::anyhow!("unknown current stage"))?;
        ensure!(
            stage.approval_required,
            "current stage does not require approval"
        );
        let next = if stage.routes.is_empty() {
            ensure!(pending.route.is_none(), "unexpected pending route");
            stage.on_pass.clone()
        } else {
            let route = pending
                .route
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("pending route missing"))?;
            stage
                .routes
                .get(route)
                .ok_or_else(|| anyhow::anyhow!("pending route is not declared"))?
                .clone()
        };
        let review_action = pending.review_action.clone();
        let route = pending.route.clone();
        self.approvals.push(Approval {
            approval_id: pending.approval_id.clone(),
            review_action: pending.review_action.clone(),
            stage: self.stage.clone(),
            commit: commit.to_owned(),
            snapshot_digest: digest.to_owned(),
            approved_at_unix,
            note,
        });
        self.pending_approval = None;
        self.advance(workflow, &next, &review_action, route.as_deref());
        Ok(())
    }
    fn advance(
        &mut self,
        workflow: &Workflow,
        next: &str,
        review_action: &str,
        route: Option<&str>,
    ) {
        if workflow
            .max_stage_transitions
            .is_some_and(|max| self.stage_transitions >= max)
        {
            self.status = Status::Stopped;
            self.stop_reason = Some("workflow stage transition limit reached".into());
            return;
        }
        let Some(count) = self.stage_transitions.checked_add(1) else {
            self.status = Status::Stopped;
            self.stop_reason = Some("workflow stage transition counter overflow".into());
            return;
        };
        // A nonterminal stage cannot ever finish if entering it uses the last
        // available transition. Stop at its predecessor before dispatching any
        // more work or external side effects. Completion may use the last one.
        if next != "complete"
            && workflow
                .max_stage_transitions
                .is_some_and(|max| count >= max)
        {
            self.status = Status::Stopped;
            self.stop_reason = Some("workflow stage transition limit reached".into());
            return;
        }
        self.stage_transitions = count;
        self.last_transition = Some(Transition {
            from_stage: self.stage.clone(),
            to_stage: next.to_owned(),
            reviewed_commit: self.accepted_commit.clone(),
            review_action: review_action.to_owned(),
            selected_route: route.map(str::to_owned),
        });
        if next == "complete" {
            self.status = Status::Complete;
        } else {
            self.stage = next.to_owned();
            self.gate = Gate::Worker;
            self.review_cycles = 0;
            self.feedback.clear();
            self.unblock_used = false;
            self.status = Status::Ready;
        }
    }
    fn block(&mut self) {
        if self.unblock_used {
            self.status = Status::Stopped;
            self.stop_reason = Some("repeated block after retry".into());
        } else {
            self.unblock_used = true;
            self.unblock_origin = Some(self.gate);
            self.gate = Gate::Unblocker;
        }
    }
}
