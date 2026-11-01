# V0 implementation plan

**Implementation status: not started.** This plan is sequenced to preserve the working Orchestrate Build product while proving the generic engine on a real Magic simulation pipeline.

## Stage 0 — Baseline and characterization

**Goal:** know exactly which existing behavior is being reused.

- Pin an Orchestrate `main` commit for the work and inventory state, controller, adapters, packet builder, CLI and core interfaces.
- Run current tests; capture what they exercise and where live provider execution is not covered.
- Characterize Work complete → Review pass, Review changes_required → Work, Work/Review blocked → Unblock → retry/stop, reset_required, resumed fresh process and Git commit invariants.
- Record existing Build's schema numbers and compatibility promises separately; do not migrate state in place.

**Deliverable:** test inventory, reuse map, baseline green CI/test record.

## Stage A — New generic workflow crate and frozen template

**Goal:** represent a workflow as data rather than Rust gate enums.

- Add `crates/workflow` with schema-versioned `WorkflowDefinition`, `RoleDefinition`, `StageDefinition`, `Transition`, and durable `WorkflowRunState`.
- Parser validates names, references, reachable terminal path, allowed pass routes, bounded recovery, provider settings, declared inputs/outputs, path confinement and instruction references.
- Implement snapshot and digest of the complete template **plus referenced authoritative files** at run initialization.
- Add tests that reject malformed/unreachable/mutated templates.
- Keep `crates/build` intact; share only proven primitives from `crates/core` or small extracted modules.

**Exit:** a validated definition creates immutable run identity and a durable `ready` state without starting any model.

## Stage B — Deterministic gate runner and Git candidates

**Goal:** execute one stage, check its outputs, and persist state safely.

- Reuse the shared provider adapter seam and action packet/invocation receipt mechanics.
- Add a command worker adapter with the same response envelope; clarify who commits artifact outputs (recommended v0: Worker/adapter produces commit, engine only validates it).
- Create controlled run worktrees, record baseline/current candidate/accepted position, verify commit ancestry and clean worktree.
- Validate required Git-visible output paths and optional JSON schemas at the exact candidate commit.
- Implement Reviewer/verification packet and exact-candidate isolation.
- Expose `run`, `status` and structured event output.

**Exit:** two-stage happy path completes through one CLI launch with reproducible output identity.

## Stage C — Feedback, Unblocker, and interruption recovery

**Goal:** reproduce the value of existing Orchestrate beyond the happy path.

- Route `changes_required` to same-stage Worker with full feedback, bounded cycles and preserved candidate lineage.
- Route semantic `blocked` to one Unblocker detour; deliver originating gate and exact context, then retry or stop.
- Persist action packet/argv before `running`; stop on malformed response, provider failure, crash, or unexpected Git state.
- Implement explicit `resume` and `reset` with worktree ownership/commit verification and destructive-action warnings.
- Exercise a genuine fresh-process resume (not merely mocked state in memory).

**Exit:** failure matrix in [acceptance](acceptance.md) passes, including repeated blocker and crash while running.

## Stage D — Model-facing authoring and domain validation

**Goal:** prove the workflow *definition* changes per problem while the code remains constant.

- Add a versioned authoring guide or thin skill (`flow author` or similar), plus `flow validate` diagnostics actionable by a model.
- Build a real Magic deck simulation template using existing simulation commands (no fake “simulate” shell echo as the sole proof).
- Make one Reviewer intentionally request changes, one Worker deliberately block, and use Unblocker retry.
- Create a second materially different example (e.g., data validation/reporting) and run it with unchanged engine code.
- Validate cost intent by assigning cheap/provider-specific routine workers and more capable reviewers only where required; do not claim measured savings unless tracked.

**Exit:** both workflows satisfy criteria and the existing Orchestrate Build suite remains passing.

## Proposed CLI

```text
orchestrate flow validate <path-to-template>
orchestrate flow run <path-to-template> [--project <repo>]
orchestrate flow status <run-id> [--json]
orchestrate flow resume <run-id>
orchestrate flow reset <run-id>
```

Syntax is proposed, not implemented. Avoid a separate command per stage; the engine should continue autonomously until completed, blocked, or needing explicit reset.

## Recommended internal seams

1. `DefinitionLoader` / `ValidatedDefinition`: pure validation and frozen references.
2. `RunStore`: durable read/write of current run state plus append-only diagnostics.
3. `RoleInvoker`: agent or command invocation, exact receipt, response extraction.
4. `ArtifactValidator`: existence, schemas and allowed path/commit checks.
5. `GitCheckpoint`: branch/worktree management, candidate/accepted checkpoint validation/reset.
6. `TransitionEngine`: pure routing on validated outcomes plus bounded counters.
7. `Observer`: events for CLI without routing authority.

Do not add abstractions solely because they appear attractive; each seam should have a testable independent responsibility.

## Delivery guidance

Prefer small PRs aligned to the above stages; each PR should include tests and synchronized docs. Avoid one mega-refactor that moves Build first and introduces a generic engine later. The value of v0 is a new working engine **alongside** the existing product, not theoretical generality.

## Definition of done

See [acceptance matrix](acceptance.md). A passing happy path alone is insufficient. This feature exists to make failure handling reliable.
