# Goals, requirements, and boundaries

**Status:** v0 scope proposal. The overall direction—Orchestrate as baseline; model-authored workflows; Git checkpoints—is established. Individual implementation details remain subject to validation.

## Functional requirements

| ID | Requirement | Evidence expected |
| --- | --- | --- |
| F01 | A workflow template describes named stages, roles, referenced instructions, inputs, outputs, and legal successor states. | Validated example; parser tests |
| F02 | The same runtime executes different templates without source changes. | Two-domain integration test |
| F03 | Roles select provider, optional model, effort, native args, and profile; stage-specific assignment is supported. | Invocation-record tests |
| F04 | Agent workers and deterministic command workers can both perform a stage. | End-to-end examples |
| F05 | A Worker candidate is mechanically checked before a Reviewer is invoked. | Invalid schema/missing file tests |
| F06 | Reviewers return constrained pass/changes_required/blocked outcomes; the engine routes those results but does not independently adjudicate domain truth. | Feedback and routing tests |
| F07 | Corrective feedback is delivered to the originating Worker for the same stage/candidate lineage. | Loop test |
| F08 | An Unblocker receives originating gate, exact failure and feedback; retries are bounded. | Blocker tests |
| F09 | The engine preserves durable state, action identity, template identity, Git commit references, and action logs. | Restart tests |
| F10 | Run/resume/status/reset are available through one CLI surface; no per-stage coordinator prompts are required. | CLI smoke tests |
| F11 | A model can author the domain input/output contract before execution; an active run cannot silently change it. | Freeze/mutation tests |
| F12 | The existing engineering Build continues to work unchanged during v0 validation. | Legacy regression suite |

## Safety and reliability requirements

- **Deterministic authority:** Rust alone mutates workflow position, statuses, checkpoints, and legal transitions.
- **Frozen execution contract:** The initial template and authoritative referenced instruction/schema content are bound by digest or immutable snapshot before execution; live edits are rejected or ignored for that run according to a documented policy.
- **Fail closed:** Invalid role response, unknown outcome, wrong action identity, invalid candidate commit, missing declared output, or failed mechanical gate cannot advance the run.
- **Correct evidence scope:** Reviewer inspects the exact candidate; not latest mutable branch state or an unconstrained alternative file.
- **Bounded recovery:** No recursive unblock loops and no unbounded review cycles by default.
- **No invented recovery:** A crash during a side-effectful action stops as uncertain and requires explicit recovery/reset.
- **Local and single-operator initially:** No scheduler, service, distributed locking, or parallel agent runs in v0.

## Exclusions from v0

No visual editor; multi-host execution; cloud scheduling; arbitrary user-defined Rust plugins; dynamic unapproved graph modification; multi-agent consensus; provider auto-selection; granular spending meter; broad external APIs; automatic compensation for network/database side effects; or mandatory migration of the existing Orchestrate Build controller.

No promise that a valid JSON schema proves domain correctness: it establishes shape and existence, not that the simulation or analysis is scientifically valid. Domain truth remains a Reviewer judgment plus any deterministic checks the template supplies.

## Constraints worth challenging during development

- **Does every stage require AI review?** No. Allow an explicit deterministic verifier option, but never interpret “no reviewer” as “no gate.”
- **Does every stage need a Git commit?** For v0 yes when a stage produces repository artifacts; command workers must follow the same candidate/checkpoint contract. Read-only stages can be represented through versioned evidence artifacts committed to the run branch. Evaluate ergonomics in the prototype.
- **Are arbitrary graph cycles necessary immediately?** Probably not. v0 supports named pass transitions and built-in correction/unblock loops; extra graph cycles are deferred unless the two-workflow test proves them necessary.
