# Instructions for implementation agents

This repository contains a **partial implementation**, not a working generic runtime. The CLI validates and snapshots definitions; the examples are not executable. See [implementation status](docs/implementation/status.md). Read [README.md](README.md), [scope](docs/scope.md), and [decision records](docs/decisions/README.md) before proposing changes.

## Principal objective

A stable Rust execution engine must run new model-authored workflows without requiring engine-code edits. It must retain Orchestrate's durable Worker/Reviewer/Unblocker feedback and Git-checkpoint safety.

## Rules

- Treat `direction agreed` ADRs as user decisions. Treat `proposed` entries as recommendations to test, not settled authority.
- Preserve existing Orchestrate Build behavior until the generic prototype passes its own acceptance cases.
- Inspect real upstream repository source before implementing; this planning repo was grounded against Orchestrate `main` revision `281af897` on 2026-11-01 and may become stale.
- Do not substitute broad workflow-platform features for the narrow v0 requirements.
- Do not allow a Worker/Reviewer to change transition authority by prompt, free-form text, or in-flight template edits.
- Do not present a Git reset as equivalent to undoing external side effects.
- Require mechanical evidence before advancing and independent domain review where declared.
- Keep reusable template syntax, test fixtures, docs, and acceptance criteria synchronized.
- Run tests and report exactly what was executed and any limitations.

## Starting work

Follow [implementation plan](docs/implementation/plan.md). First characterize the current Build controller and regression tests. Only then stabilize the template schema and create new runtime code. Use [acceptance matrix](docs/implementation/acceptance.md) as the definition of done.
