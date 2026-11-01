# Template authoring, validation, and freezing

## Authoring loop

1. User specifies the outcome and references existing tools/repository resources.
2. Authoring model examines available contracts and asks only for decisions needed to avoid incompatible assumptions.
3. Model drafts a template, role profiles, stage instructions, inputs/outputs, verifiers and acceptance criteria. It can choose inexpensive agents for routine steps and stronger models for judgment.
4. `orchestrate flow validate <template>` returns deterministic errors. The model iterates **before launch** until validation passes.
5. User reviews the proposed intent and authorizes the run. Initialization snapshots the template and authority files, pins input/baseline Git identity, and records a durable run.
6. Runtime uses only the frozen contract. When genuinely necessary, user changes result in a **new version/new run** (or an explicitly designed, separately validated amendment protocol after v0), never an in-place rewrite.

## Authority ranking

User intent and approvals govern the author's design. The frozen template defines machine behavior after launch. Stage instructions explain *what the agent should accomplish*, but never override the contract's allowed outcome names, exact action identity, checkpoint rules, or other runtime invariants.

A model that authored the workflow cannot silently loosen a schema because a Worker produced invalid output. A Worker cannot invent an unlisted stage. A Reviewer cannot alter the accepted inputs after seeing the results. The controller cannot invent a semantic PASS from a schema-valid file.

## Profile contents

Each profile should specify purpose, permitted tools, expected work boundaries, output behavior, and what evidence to include in the report. Keep reusable profile rules distinct from a stage's task requirements. When provider permissions are consequential, validate native adapter/sandbox rules mechanically; prompt instructions alone are not access controls.

## Template reuse

Template definitions can be checked into a repository. Each run is pinned to a template digest plus an input baseline. A newer model-authored template version does not affect already-running executions. Preserve old run metadata and action reports for comparison.

## Authoring guide deliverable

Provide a dispatcher-style skill or guide consistent with Orchestrate practice: it directs a model to collect requirements, inspect tools, write the template and docs, validate them through CLI, and stop for launch authorization. Keep deep guidance with the versioned CLI/docs instead of copying mutable policies into every installed skill.
