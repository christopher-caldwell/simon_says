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
                            });
                            self.status = Status::AwaitingApproval;
                        } else {
                            self.advance(&stage.on_pass);
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
        let next = stage.on_pass.clone();
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
        self.advance(&next);
        Ok(())
    }
    fn advance(&mut self, next: &str) {
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
