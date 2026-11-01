# Execution, feedback, and recovery policy

**Primary requirement:** preserve Orchestrate's reliable Worker ↔ Reviewer correction and bounded Unblocker detour; do not replace it with an AI coordinator.

## Durable state (proposed)

At minimum: schema version; `run_id`; template snapshot/digest; starting commit; current stage name; current gate; `ready|running|stopped|complete` status; accepted commit and candidate commit (or an unambiguous equivalent); current action ID; action/feedback references; per-stage review cycle count; unblock provenance/attempt count; stop reason; and completion reference.

The journal and action logs aid inspection. **The durable state record and frozen contract determine the next route**; the engine should never infer routing from the last prose message or search historical logs for a convenient outcome.

## Standard stage routing

```text
Worker --complete--> mechanical checks --> Reviewer/verification
   |                                         |  pass --> accept stage --> on_pass
   |                                         |  changes_required --> Worker (same stage)
   |                                         `- blocked --> Unblocker
   `- blocked --> Unblocker -- retry --> original gate
                                     `- blocked --> durable stop

malformed output / provider failure / unexpected Git state
  --> reset_required (explicit operator recovery)
```

### Why `blocked` differs from failures

`blocked` is a valid role outcome: the role finished, supplied the exact action ID, and reported an inability to continue based on semantic/task conditions. That permits a bounded Unblocker analysis. A provider crash, missing response, malformed JSON, wrong action ID, or unexpected Git mutations is **not** a valid `blocked` verdict. It is an uncertain/protocol failure and must stop for reset/inspection.

### Reviewer corrections

A `changes_required` result leaves the stage active and forwards the entire report to the Worker. It doesn't erase previously produced artifacts, select a different template, or give the Reviewer independent transition authority. Default v0 review limit: three corrections per stage, configurable and bounded; reaching the limit stops with evidence. Count exact cycles, not user prompts.

### Unblocker detour

Keep Orchestrate's conservative behavior: one automatic Unblocker visit per blocked attempt at an originating gate. If it returns `retry`, restore the correct safe Git checkpoint and requeue the original gate with original feedback and Unblocker diagnosis. If the role blocks again without progress, stop; do not recursively call Unblocker. An explicitly resumed run may begin a new bounded attempt after operator correction, in line with existing Build semantics.

### Explicit recovery after process interruption

Before invoking an action, write the action packet and invocation record, then durably mark `running`. If the process ends while `running`, a new CLI invocation must not assume the action failed harmlessly or finished successfully. Report `reset_required`, the current action, its working tree, known Git state, and what a reset would discard.

Keep `resume` and `reset` separate:

- `resume` continues a clean `ready` or explicitly resolved `blocked` state; it never silently cleans ambiguous artifacts.
- `reset` requires explicit action; it verifies the checkpoint and controlled worktree and requeues the originating gate. Explain destructive Git effects before performing them.
- If a previous command already performed external effects, the operator must reconcile those separately.

## Machine-enforced failures

Wrong action ID, schema mismatch, missing output file, unclean `HEAD`, commit outside ancestry, unauthorized template modification, or review worktree mutation cannot be waived by an agent's narrative. Stop or requeue only according to the documented mechanical policy. Model-proposed repairs can be useful for diagnosis, but they must not masquerade as successful verification.

## Required observability

For each action persist the role identity, selected provider/model/effort, exact argv, action packet, raw transport/logs, parsed result, report, associated candidate commit, and routing event. `status` should distinguish `candidate`, `last accepted`, `gate`, `blocked`, and `reset_required` without reading source code.
