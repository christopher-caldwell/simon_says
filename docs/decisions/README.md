# Decision records

Decision records preserve the product direction and assumptions without making proposed implementation details look approved.

| Record | Status | Summary |
| --- | --- | --- |
| [ADR-0001](0001-orchestrate-baseline.md) | Direction agreed | Base the generic runtime on Orchestrate's durable gate model, not Control Tower/Dagu |
| [ADR-0002](0002-git-checkpoints.md) | Direction agreed | Keep Git as the v0 checkpoint substrate |
| [ADR-0003](0003-authoring-vs-runtime.md) | Direction agreed | Model-authored workflow, deterministic runtime authority and frozen contracts |
| [ADR-0004](0004-sibling-runtime-first.md) | Proposed | New crate alongside existing Build, prove before migration |

For unresolved design details see [open questions](../implementation/open-questions.md). Do not amend a past decision retroactively to make later implementation appear authorized.

- [ADR-0005: Separate runtime repository](0005-separate-runtime-repository.md) — implementation decision adapting the sibling-crate proposal to separate repository ownership.

- [ADR-0006: Workflow-defined role routing](0006-workflow-defined-role-routing.md) — user direction for authored roles and dynamic dispatch; bounded reviewer-selected routes are the initial implementation.
