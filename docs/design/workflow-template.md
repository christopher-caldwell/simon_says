# Workflow template contract

**Status:** illustrative v0 format proposal, NOT finalized parser syntax. The implementation phase must stabilize the schema and test it before claiming compatibility.

## Distinct objects

- **Template:** reusable ordered, named stage graph and role definitions; checked in or created by a model; versioned.
- **Run:** one invocation of a template against a specific repository baseline and declared inputs; stateful and immutable in identity.
- **Stage:** one domain task with its contract and accepted transition.
- **Gate:** an action within a stage (Worker → mechanical checks → Reviewer, or deterministic verification).
- **Action:** one concrete, uniquely identified role/command invocation with recorded inputs and response.

## Proposed minimal TOML example

```toml
schema_version = 1
name = "deck-simulation"
start = "simulate"

[roles.simulator]
kind = "command"
argv = ["./scripts/simulate"]

[roles.analyst]
kind = "agent"
adapter = "codex"
effort = "high"
profile = "agents/analyst.md"

[roles.reviewer]
kind = "agent"
adapter = "codex"
effort = "medium"
profile = "agents/reviewer.md"

[roles.unblocker]
kind = "agent"
adapter = "codex"
effort = "high"
profile = "agents/unblocker.md"

[stages.simulate]
worker = "simulator"
reviewer = "reviewer"
unblocker = "unblocker"
instructions = "stages/simulate.md"
inputs = ["decks/current.json", "config/simulation.json"]
outputs = ["results/simulation.json"]
output_schema = "schemas/simulation.schema.json"
on_pass = "analyze"
max_review_cycles = 3

[stages.analyze]
worker = "analyst"
reviewer = "reviewer"
unblocker = "unblocker"
instructions = "stages/analyze.md"
inputs = ["results/simulation.json"]
outputs = ["results/analysis.json"]
output_schema = "schemas/analysis.schema.json"
on_pass = "complete"
max_review_cycles = 3
```

This example assumes a command adapter that returns the standard action envelope and produces a Git commit; simple shell scripts may need a thin adapter script. The domain-specific scripts are existing user-owned tools, not engine code.

## Stage semantics

V0 supports unique named stages and an explicit `on_pass` destination (another stage or reserved `complete`). A stage may also declare route labels mapped to destinations; a passing Reviewer selects a declared label and the controller resolves it. Named routes or cycles require a finite `max_stage_transitions`. Built-in bounded `changes_required → same Worker` and `blocked → Unblocker → originating gate` routes remain separate. A stop is represented by durable run status, not a guessed route. Arbitrary condition expressions, nested graphs and parallel branches remain out of scope. See [ADR-0006](../decisions/0006-workflow-defined-role-routing.md).

Reviewer defaults to an explicit named role. A deterministic verifier may replace the Reviewer when its exit/result protocol is documented; omitting a Reviewer without declaring a verifier is invalid. All stages have a mechanical output check whether or not they have an AI Reviewer.

The action protocol has three fixed gate types (`worker`, `reviewer`, `unblocker`); these are not a fixed catalog of domain roles. Each workflow defines its own named roles, profiles, models, and stage assignments. A named role such as a simulator, investigator, or fixer can be assigned to a stage, and declared routes determine which stage runs next.

## Preflight validation

Reject before launch:

- Invalid schema version, unknown mandatory keys, duplicate stage/role names, invalid identifiers.
- Missing entry stage; unknown targets; unreachable stages; no route to completion; unbounded conditional routes or cycles.
- Missing instructions, profiles, schema files, or declared starting inputs.
- A reference escaping the workflow/template/run root, untrusted symlinks in authoritative paths, or absolute paths when the policy requires relative paths.
- Unsupported provider/model/effort combination where the adapter can detect it; other provider failures are reported at invocation.
- A stage without a Worker or without a review/verification gate.
- Missing limits or impossible bounds for retries/corrections.
- An inconsistent or dirty source baseline as defined by [Git checkpoint rules](git-checkpoints.md).

## Explicitly model-authored data contract

A model chooses domain input/output names and can produce JSON Schema files and Markdown acceptance criteria. The engine checks path existence, allowed locations, type/schema (where specified), hashes, action identity, and commit provenance. It does **not** interpret words such as “the simulation is statistically reliable” as machine-checkable unless a verifier has been supplied.

For opaque or non-JSON outputs, the template must specify the mechanical checks available (for example existence plus a command verifier). File globbing, large binary artifacts, datasets outside Git, and references to remote object stores are outside the first proof unless specifically required by the Magic pipeline.

## Transition table (v0)

| Gate | Valid result | Next action |
| --- | --- | --- |
| Worker | `complete` | Validate candidate, then Review/verify |
| Worker | `blocked` | Invoke bounded Unblocker |
| Reviewer | `pass` | Accept stage; follow `on_pass` or the declared label selected by the Reviewer |
| Reviewer | `changes_required` | Same stage Worker with feedback |
| Reviewer | `blocked` | Invoke bounded Unblocker |
| Unblocker | `retry` | Requeue originating gate with context |
| Unblocker | `blocked` | Durable stop |
| Any | malformed/protocol error | Durable reset-required stop |

No arbitrary “next_stage” field in an agent result. A passing Reviewer may select a route label declared at authoring time; it cannot supply a destination or change the frozen map.

## Template version identity

Store schema version, template bytes/hash, referenced file digests, stage-role assignment, and the source baseline. A run either uses an immutable copy of all references or checks a manifest of their exact bytes on every resume. **Preferred v0:** snapshot the complete authoritative definition before the first action, then use only that snapshot for the active run; edits to the reusable template affect only future runs.
