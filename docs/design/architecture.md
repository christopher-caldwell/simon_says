# Proposed architecture

**Status:** proposed v0 architecture. It is derived from the existing Orchestrate Build implementation, not a statement that this code already exists.

## Layering

```text
Model authoring tool / skill
         |
         v
   workflow definition + profiles + contracts
         |
         v
   load / validate / freeze --------------> immutable run definition
         |                                         |
         v                                         v
CLI (run, resume, status, reset) ------------> Workflow controller
                                                  |        |
                        frozen transition table --+        +-- durable state
                                                  |
                            action packet -> role invoker
                                                 /   \
                                         command   provider adapters
                                                 \   /
                                    response / artifacts / Git commit
                                                  |
                                    mechanical validator
                                                  |
                                     route and checkpoint
```

## Ownership boundaries

### Authoring model

May propose stages, agent profiles, tasks, required inputs and outputs, Reviewer instructions, allowed result routes, and bounds. May iterate on an unlaunched template in response to validation errors. Does **not** own runtime state or accept its own output after launch.

### Generic Rust controller

Loads and freezes the template; assigns a unique action ID; constructs precise action packets; invokes roles; validates response envelopes, declared artifacts, and Git invariants; records checkpoints and events; applies only permitted transitions. It doesn't decide whether a deck simulation or engineering review is substantively correct.

### Role worker

An agent or a command that executes exactly the current assigned task. Its valid result includes the action identity, outcome, report, and a candidate commit on completion. A worker does not select the next stage.

### Reviewer

An independent role that evaluates the exact candidate against the stage's acceptance contract. Returns `pass`, `changes_required` with feedback, or `blocked`. The controller checks identity, allowed outcome, and mechanical invariants—not the truth of the reviewer's conclusion.

### Unblocker

A separate role invoked when a Worker or Reviewer explicitly reports `blocked` (or when a configurable policy treats missing domain evidence as blocked). It gets originating gate, stage, checkpoint, inputs, feedback and failure context. It may recommend `retry` or return `blocked`. It cannot silently rewrite the template or override a mechanical failure. Mechanical/protocol failures require deterministic reset, not an AI Unblocker diagnosis passed off as authority.

## Reuse vs extraction

Inspect these existing sources first:

- [Current Build state](https://github.com/christopher-caldwell/ochestration/blob/main/crates/build/src/state.rs): fixed `Gate` enum, `BuildPlan`, `BuildConfig`, `BuildState` and checkpoint fields.
- [Build controller](https://github.com/christopher-caldwell/ochestration/blob/main/crates/build/src/controller.rs): action lifecycle, Work/Review/Audit/Unblock routing, reset policy.
- [Action packet builder](https://github.com/christopher-caldwell/ochestration/blob/main/crates/build/src/packet.rs): gate-specific prompts and response shapes.
- [Build adapter](https://github.com/christopher-caldwell/ochestration/blob/main/crates/build/src/adapter.rs): existing native provider interface and policy.
- [Core provider module](https://github.com/christopher-caldwell/ochestration/tree/main/crates/core): shared provider invocation machinery.

Suggested extraction boundaries:

| Reuse or generalize | Keep engineering-specific |
| --- | --- |
| Provider process invocation, native args, session extraction, logs | Build phase prose and engineering-specific guide text |
| Action identity, state persistence, response envelopes | Reconciled Discovery/Adoption/Implementation/Audit artifact contracts |
| Git candidate/HEAD/ancestry/worktree mechanics | Build-specific repository cleanliness and accepted engineering semantics, where different |
| Feedback persistence, bounded correction, Unblock detour | Fixed `Work`, `Review`, `Audit` routing and phase indexing |
| CLI observer/events and diagnostic files where reusable | Current Build status bar and phase-based counters |

Do not blindly move the 3,000+ line Build controller into a generic crate: first isolate a small generic interface in parallel. Keep the old controller as a consumer or separate implementation until equivalence has been demonstrated.

## Suggested crate and CLI shape

```text
crates/
  core/              # Existing shared provider machinery
  build/             # Existing, initially unchanged
  workflow/          # New: template, state, runner, routing, gates
  cli/               # New `orchestrate flow ...` commands
```

A `WorkflowDefinition` is **configuration**, not active state. A `WorkflowRun` is **durable per-execution state**, with a frozen definition identity and Git checkpoint references. The runner should not ask a model to determine which step was last completed.

## Design invariants

1. Only the controller can transition between states.
2. No transition before durable evidence and mechanical validation.
3. A Reviewer judges a precise candidate, not a moving branch.
4. Feedback and checks are part of the same action lineage.
5. Frozen inputs are reproducible and never silently replaced by agent preference.
6. An unknown or ambiguous side effect cannot be declared safe to repeat merely because a process restarted.
7. Existing Build contracts remain unchanged unless a separately reviewed migration is undertaken.
