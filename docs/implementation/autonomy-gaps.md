# Autonomy: reassessment of the Magic study

Status: open. The intended product is a generic dispatcher driven by an authored
workflow. The workflow defines roles, their jobs, and allowed handoffs; models
perform those jobs. Rust invokes the configured roles, carries the recorded
context, validates the execution protocol, and applies configured transitions.
A supervisor is a role in this Magic workflow, not a built-in Simon Says role.

## What the Magic workflow actually configured

The OpenAI continuation workflow (`magic-optimization-openai-run-v2`) ran the
supervisor as a stage before a separate command-based collection stage. The
collection stage's Unblocker was a command explicitly configured to stop. It
had no configured handoff to the supervisor on failure. The analyzer could select
a declared route to the fixer, but that route occurred after collection, and
the repair stage ended the workflow rather than returning to collection.

Consequently, even if the collection command had returned a valid blocked
response, this definition would not have produced the requested loop in which
the supervisor responds to an error, routes work to a fixer, and continues the
study. That is a workflow design error. I compounded it by manually authoring
and starting two separate repair workflows instead of arranging the intended
handoffs in the study workflow.

## What failed in the run

- The Magic collection script raised a `KeyError` after preparing assignments
  and before starting games. This is a Magic workflow defect.
- Its blocked response included a field prohibited for a Worker. The
  controller rejected the envelope and stopped before invoking the configured
  Unblocker. That configured Unblocker would have stopped anyway; this run
  does not establish that Simon misrouted a valid failure.
- The run records 20 prepared CP3 runs and 1,000 pending trials, with zero
  attempts at inspection. Two subsequently launched repair workflows produced
  a reviewed script fix and a read-only reconciliation, but did not resume
  the study. No conclusion about deck optimization resulted.

## Framework questions and remaining verification

The controller currently supports named workflow roles, stage transitions,
reviewer-selected declared routes, and bounded Worker/Reviewer/Unblocker
handling. Currently only a passing Reviewer can select a declared route; a
Worker or Unblocker cannot select one directly. Its Unblocker runs with
read-only intent and cannot commit a repair. These are current implementation
constraints, not requirements that every workflow have a supervisor. Whether the existing stage and route mechanisms can express the complete Magic
supervisor → fixer → supervisor → retry cycle with the necessary context must be
established with a small representative workflow and tests. If not, any added
routing capability must be generic: Simon must not diagnose Magic failures or
assume a supervisor or fixer exists in other workflows.

A malformed protocol response currently stops the run rather than reaching a
configured role. A bounded, generic way to make response errors observable to
an authored role may be useful, but the observed failure alone does not prove a
particular controller change is required: first establish whether an appropriate
workflow can report and handle the underlying error with a valid envelope.
Preserve action identity and avoid silently treating an invalid response as a
valid outcome.

The requested end-to-end behavior remains unverified: a supervisor observing a
real collection problem, a workflow-dispatched fixer addressing it, and the
study continuing to a result without an operator creating intermediate
workflows. Several sequential unattended research scenarios are also unverified.
Use cheap deterministic fixtures first, then a bounded real run; stop broken
paid work rather than allowing it to consume tokens without a result.

## Evidence

The following run directories are siblings of the `simon_says` checkout:

- `magic-optimization-openai-run-v2/worktree/continuation.toml`: configured
  roles, stages, and transitions. `state.json` records the stop at collection.
- `magic-optimization-openai-run-v2/actions/action-4/`: collection response and
  journal. The native database held the prepared trials and zero attempts at
  inspection.
- `magic-optimization-repair-run-v1/`: reviewed script repair at commit
  `ba16a08179494ccb643e7af4b7f79cbb146b6a92`.
- `magic-optimization-recovery-run-v1/`: reviewed read-only reconciliation at
  commit `62552dd61d5f5ecc9ebe1b62159e2d530d0b3242`.
