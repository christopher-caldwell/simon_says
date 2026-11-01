# Agents, prompts, and contracts

## One generic protocol, multiple task profiles

Each role references a model-facing profile (`.md`), and each stage references task instructions and acceptance criteria. The reusable runtime applies the same response protocol regardless of whether the domain is code, Magic deck simulation, data ETL, or document production.

Roles can be assigned different provider adapters, models, effort levels, and native arguments. `effort` has provider-specific semantics; use the adapter behavior already established by Orchestrate rather than inventing cost or capability guarantees.

## Action packet

Create a packet **before invocation**, associate it with a unique `action_id`, and persist it alongside the exact provider argv and action logs. Proposed packet fields:

```json
{
  "schema_version": 1,
  "run_id": "<opaque-run-id>",
  "action_id": "<opaque-action-id>",
  "template_digest": "<digest>",
  "stage": "simulate",
  "gate": "worker",
  "role": "simulator",
  "baseline_commit": "<git-oid>",
  "candidate_commit": null,
  "inputs": [{"path": "decks/current.json", "sha256": "<digest>"}],
  "expected_outputs": [{"path": "results/simulation.json", "schema": "schemas/simulation.schema.json"}],
  "instructions": "<frozen-stage-instructions>",
  "feedback": [],
  "allowed_outcomes": ["complete", "blocked"]
}
```

Exact serialized details are for implementation. Never rely on conversation history to reconstruct an action packet after restart.

## Mandatory outer response envelope

All agents/commands return one response object with the **current** `action_id`, an allowed outcome, and a nonempty human-readable report. Completed Worker responses additionally name the candidate commit. Reviewers name the exact commit inspected. An Unblocker returns `retry` or `blocked` with a report. Use strict parsing with unknown fields rejected unless explicitly versioned.

The domain artifact format is *not* fixed by this envelope. For Magic, the model may choose `results/simulation.json` and `results/analysis.json`; for ETL it might choose different outputs. The controller verifies against the frozen stage contract, not guessed filenames.

## Mechanical versus semantic verification

Mechanical checks (Rust or explicit command verifier): valid action ID, response schema, declared files, JSON Schema compliance where configured, Git ancestry and exact `HEAD`, clean worktrees, path confinement, required evidence references, and supported next route.

Semantic judgment (Reviewer): whether the requested simulation actually used correct methodology, whether the result addresses the task, whether omissions or limitations are acceptable. A reviewer can be wrong; the runtime enforces *process*, not infallibility.

A deterministic verifier's exit code is evidence only for what its assertions genuinely check. A successful verifier does not establish unrestricted domain truth.

## Review packets

Reviewer always starts from the exact candidate checkpoint in an isolated detached read-only-intent worktree and gets the original requirements, output paths, worker report, and all relevant correction feedback. The Reviewer does not mutate the candidate; detect and reject unexpected changes in the review worktree.

On `changes_required`, preserve structured and human-readable feedback in the run directory. Forward the complete feedback and candidate lineage to the same-stage Worker, which may continue from the appropriate candidate rather than restarting the entire workflow.

## Session policy

Preserve Orchestrate's safe default: Worker sessions may be retained **within one process and stage**; Reviewer and Unblocker start fresh. Sessions are not part of durable state. A resumed process rebuilds full action packets and starts a new provider conversation. Explicitly reset Worker session identity after an accepted stage.

## Privileges and isolation

Model role assignment does not itself constrain file or network access. Retain provider-specific tool/sandbox policies and validate them independently; do not assume a prompt saying “review only” prevents writes. Allowing native arbitrary arguments can override provider policy, so the v0 documentation must plainly distinguish trusted local configuration from security-enforced permissions.
