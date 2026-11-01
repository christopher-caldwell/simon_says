# ADR-0004: Implement a sibling generic runner before migrating Build

**Status:** proposed engineering recommendation; not an explicit user decision.

## Context

The Build controller combines reusable invocation/checkpoint mechanics with reconciled engineering artifacts, Audit lineage, and Git-specific assumptions. A wholesale in-place generic refactor could destabilize the working Orchestrate pipeline.

## Proposed decision

Add a `crates/workflow` implementation alongside `crates/build`, reusing stable provider interfaces and extracting only proven common helpers. Test a Magic pipeline and one other domain with unchanged engine code; consider converting existing Build into a template only after this proof.

## Tradeoff

Temporary duplication is acceptable if it preserves a reliable reference implementation. If the two runners diverge significantly, revisit extraction after the new semantics are measured and tested.

## Follow-up evidence

See [v0 plan](../implementation/plan.md) and [acceptance matrix](../implementation/acceptance.md).
