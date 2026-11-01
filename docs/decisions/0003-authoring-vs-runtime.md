# ADR-0003: Models author contracts; controller executes frozen routes

**Status:** direction agreed, 2026-11-01.

## Context

Agent-authored instructions alone do not reliably enforce sequencing or gates. The user wants each new workflow to have its own model-designed inputs/outputs and agent profiles, without changing runtime code.

## Decision

Before execution, a model may decide stage structure, role configuration, required inputs/outputs, verification criteria and legal routes. At launch these definitions become a frozen contract. During execution, Rust is solely authoritative over transitions and persisted state; the Worker and Reviewer return bounded outcomes and reports, not arbitrary new workflow policy.

## Consequences

The runtime verifies mechanics but does not second-guess an independent Reviewer's domain verdict. Contract edits apply to a new run/version, not silently to an active run. Authoring skills are helpful UX but not a substitute for machine validation.

## Detail

See [authoring](../design/authoring.md) and [template contract](../design/workflow-template.md).
