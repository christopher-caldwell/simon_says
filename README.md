# Orchestrate Workflows

**Status: v0 design proposal — no runtime implementation yet.**  
**Established direction:** extend the execution model pioneered by [Orchestrate](https://github.com/christopher-caldwell/ochestration) into a generic, reusable, model-authored workflow engine. Git remains the v0 checkpoint mechanism.

## The problem

AI agents can perform substantial individual tasks, but assigning a prompt such as “follow these steps, validate the result, recover from failures, then continue” leaves the sequencing and enforcement to the model. This fails in practice: agents skip instructions, stall on environment problems, repeat work, use expensive reasoning on routine tasks, or declare success without producing valid evidence.

Orchestrate solves much of this for engineering by keeping a persistent deterministic controller outside the agents. Its **Worker → Reviewer → correction** cycle and bounded **Unblocker** detour are useful beyond software development. Today its states, artifact types, and Git rules are tied to the engineering Build/Audit workflow.

The goal here is **constant execution code, new workflow template per task**. An authoring model can define the stages, agent profiles, required inputs and outputs, checks, and permitted transitions. Rust validates and freezes the template and runs it without letting an agent become the orchestrator.

## What v0 should do

- Run model-authored, schema-versioned workflows in a local Git repository.
- Reuse Orchestrate's native AI-provider invocation patterns and bounded Worker / Reviewer / Unblocker recovery.
- Support different agent profiles, models, effort levels, and a deterministic command worker where useful.
- Validate declared artifacts and machine-readable outputs before allowing review or transition.
- Route on explicitly allowed outcomes without judging whether a reviewer's domain assessment is correct.
- Persist action identity, feedback, Git checkpoints, and interrupted/blocked state across process exits.
- Execute an entire workflow through one run/resume entry point without an AI supervisor issuing a CLI command between steps.

## What v0 is not

Not a Dagu replacement, a Control Tower replacement, a scheduler, a distributed agent system, a visual workflow editor, or a framework that promises rollback of arbitrary external side effects. The [scope document](docs/scope.md) fixes those limits.

## Repository map

| Start here | Purpose |
| --- | --- |
| [Vision and motivation](docs/vision.md) | Problem, desired experience, examples, and success conditions |
| [Goals, requirements, and boundaries](docs/scope.md) | Normative v0 scope and exclusions |
| [Architecture](docs/design/architecture.md) | Ownership, components, and separation from engineering semantics |
| [Workflow template contract](docs/design/workflow-template.md) | Proposed authorable file format and deterministic routing |
| [Agents and action packets](docs/design/agents-and-contracts.md) | Roles, domain-owned I/O, and response envelope |
| [Git checkpoint semantics](docs/design/git-checkpoints.md) | Candidate vs accepted state, commits, worktrees and limitations |
| [Failure and recovery](docs/design/recovery.md) | Feedback loop, unblock policy, crash/reset policy |
| [Authoring and freezing](docs/design/authoring.md) | Model-generated template lifecycle and authority |
| [v0 implementation plan](docs/implementation/plan.md) | Work packages, sequencing and deliverables |
| [Acceptance matrix](docs/implementation/acceptance.md) | Required mechanical, integration, and failure tests |
| [Open questions](docs/implementation/open-questions.md) | Decisions intentionally not made yet |
| [Magic deck example](docs/examples/magic-simulation.md) | Concrete motivating workflow and template sketch |
| [Existing tool comparison](docs/references/baselines.md) | What Orchestrate and Control Tower already do |
| [Decision records](docs/decisions/README.md) | Agreed directions versus unresolved implementation choices |

See [documentation index](docs/README.md) for reading order and document ownership.

## First implementation target

Implement a **two-stage Magic deck simulation** using existing scripts and agent profiles, with a failed review, worker correction, a blocked action, and an Unblocker retry. Then prove that a second, materially different workflow runs *without runtime code changes*. Keep the existing Orchestrate Build pipeline working while the new runtime is developed alongside it.

## Source and currency

This repository is a planning artifact based on discussions through **2026-11-01** and the inspected `main` revision [`281af89`](https://github.com/christopher-caldwell/ochestration/commit/281af897fcba59a4ef9476c2ce7aa40ed025b6ed) of `christopher-caldwell/ochestration`. Recheck upstream behavior before implementing: the contents are a proposed design, not claims that the generic runtime already exists.
