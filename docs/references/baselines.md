# Existing tools: factual baselines

This project follows evaluation of Dagu and Control Tower. It should not restart that evaluation without a new, concrete unmet requirement.

## Orchestrate (primary baseline)

Repository: [christopher-caldwell/ochestration](https://github.com/christopher-caldwell/ochestration). Inspected `main` at [`281af897`](https://github.com/christopher-caldwell/ochestration/commit/281af897fcba59a4ef9476c2ce7aa40ed025b6ed) on 2026-11-01.

- [README](https://github.com/christopher-caldwell/ochestration/blob/main/README.md) describes Prepare → Discovery × N → Reconcile, then explicitly authorized Adoption → Build → Implementation → Audit.
- [Architecture](https://github.com/christopher-caldwell/ochestration/blob/main/docs/architecture.md) makes Rust responsible for mechanical facts and routing, and agents responsible for engineering judgment.
- [Build state](https://github.com/christopher-caldwell/ochestration/blob/main/crates/build/src/state.rs) contains fixed `Gate::{Work, Review, Audit, Unblock}` and `Scope::{Phase, Final}`, Build plan/config and state.
- [Controller](https://github.com/christopher-caldwell/ochestration/blob/main/crates/build/src/controller.rs) routes `changes_required` to Worker and `blocked` to Unblocker, validates candidate commits and review checkpoints, and persists state.
- [Action packet](https://github.com/christopher-caldwell/ochestration/blob/main/crates/build/src/packet.rs) constructs gate-specific instructions and response schemas.
- [Build reliability lessons](https://github.com/christopher-caldwell/ochestration/blob/main/docs/evaluations/build-reliability/lessons.md) document why speculative recovery was removed in favor of explicit checkpoint/reset and a bounded Unblocker.

**Important nuance:** Build marks a mechanically valid Worker commit as its checkpoint before independent Review has passed. The generic runtime should distinguish pending candidate from accepted stage state even if it reuses the checkpoint technique.

## Control Tower (adjacent, not the selected foundation)

Repository: [christopher-caldwell/control_tower](https://github.com/christopher-caldwell/control_tower).

- [README](https://github.com/christopher-caldwell/control_tower/blob/main/README.md): a local stateful scenario workbench, intentionally not a general orchestration engine.
- [Current design](https://github.com/christopher-caldwell/control_tower/blob/main/docs/design/current-design.md): `Workspace → Workflow → Stage`; stage `up/down/verify-*` scripts and SQLite position.
- [Executable contract](https://github.com/christopher-caldwell/control_tower/blob/main/docs/reference/stage-executables.md): scripts/commands are user-owned; stdout is not a managed output handoff; exit code governs stage acceptance.

It can run the Magic scripts today, but it does not natively implement Orchestrate's Worker/Reviewer feedback and Unblocker routing. Building those in stage scripts would rebuild the same orchestration policy in a less explicit place. Its strength (simple ordered reversible stages) is not the user's most important unmet requirement (agent recovery).

## Dagu

Previously evaluated thoroughly by the project owner; Control Tower was developed as a replacement for the relevant local workbench use case. This initiative is not another Dagu implementation or comparative shopping exercise. Its focus is a **lightweight configurable agent state machine with Orchestrate-style persistent recovery**.
