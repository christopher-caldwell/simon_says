# Open questions and implementation choices

This file deliberately distinguishes **decided direction** from **proposals requiring evidence**. Do not present a recommendation below as user approval.

## Agreed direction

- Reuse Orchestrate's deterministic, durable Worker / Reviewer / Unblocker model.
- Replace hardcoded engineering-phase policy with separately authored reusable workflow templates.
- Let models design domain-specific inputs, outputs, acceptance criteria, and role profiles before launch.
- Keep the runtime code constant across distinct workflow definitions.
- Keep Git as the v0 checkpoint substrate.
- Do not rely on AI to decide transitions during execution.

## Questions to resolve in Stage 0/A

### 1. Existing Build unchanged vs gradual in-place extraction

**Recommendation:** implement a sibling `workflow` crate first and defer migration of Build. Validate reuse seams by tests. A wholesale rewrite of the existing Build controller creates unnecessary regression risk.

### 2. Run branch/worktree allocation

**Recommendation:** isolated run worktree/branch, but confirm how it interacts with user-authored commands, ignored data and tooling. Current Build has different working-tree semantics, so do not pretend this choice is already implemented.

### 3. Who commits command-worker output?

**Recommendation:** enforce one uniform candidate protocol. A command worker or adapter wrapper must produce an exact candidate commit; the Rust controller validates rather than guessing which files to commit. Check ergonomic cost against the Magic simulation scripts.

### 4. Mechanical gate failure vs semantic `blocked`

**Recommendation:** keep malformed/missing/mismatched evidence in the explicit reset/inspection lane, and reserve Unblocker for legitimate semantic task blockers. Consider whether a deterministic repair command is helpful without weakening fail-closed semantics.

### 5. Template file format

**Recommendation:** one TOML file with referenced Markdown instructions and optional JSON Schemas; prefer readability and compatibility with existing Rust tooling. The illustrative syntax is not final.

### 6. Stage graphs and conditional branching

**Resolved by user direction and [ADR-0006](../decisions/0006-workflow-defined-role-routing.md):** support workflow-declared route labels selected by the Reviewer and bounded cycles. Arbitrary condition expressions and parallel graph execution remain out of scope.

### 7. Acceptance without an AI Reviewer

**Recommendation:** allow explicit deterministic verifier as alternative, but not a stage with zero meaningful validation. Clarify representation in the stabilized schema.

### 8. Model provider safety and permission policy

**Recommendation:** reuse tested provider-native policies, explicitly define the trusted-config boundary, and verify sandbox behavior for each supported provider when introducing new role kinds. A role profile is not a sandbox.

### 9. State storage and run directory

**Recommendation:** retain straightforward local JSON state + action files initially, consistent with Orchestrate Build, unless a real concurrency or durability need justifies SQLite. Do not add a database because Control Tower uses one.

### 10. Bounded review attempts

**Recommendation:** explicit `max_review_cycles` with a safe finite default. A blocked semantic detour remains one automatic visit per attempt; on repeated stop, require operator intervention.

## Explicit later scope

Remote workers, scheduled runs, multi-lane parallel execution, conditional selectors, nested flows, external compensation, persistence migrations, automatic prompt optimization, role selection by model at runtime, cost tracking, UI, and moving current Build to a generic checked-in template.

## Risk register

| Risk | Consequence | V0 mitigation |
| --- | --- | --- |
| Generalizing everything from Build | Controller becomes harder to reason about | Narrow v0 role/transition protocol; sibling crate |
| Mutating template mid-run | Results lose authority | Frozen snapshot + digest |
| Mixing candidate with accepted checkpoint | Wrong retries or false completion | Explicit gate/status/checkpoint semantics |
| Git reset in user's active checkout | Irrecoverable local data loss | Controlled run worktree and explicit reset guard |
| Agent self-reports proof without evidence | False positive gate | Separate machine checks and Reviewer task |
| Treating subprocess success as domain truth | Incorrect simulation outputs | Contract-specific verification and independent Review |
| Recovery after external effects | Duplicate or irreconcilable actions | Explicit stop; document compensation as outside v0 |
| Role configuration overrides permission baseline | Unintended tool access | Validate native policy and document trusted configuration |
| No second-domain trial | Framework only works for Magic | Mandatory different-domain acceptance |
