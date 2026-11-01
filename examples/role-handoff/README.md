# Deterministic role-handoff fixture

This local example demonstrates role separation: Investigator preserves the frozen original question and evidence; Supervisor records a nonblocking issue backlog and collection evidence without writing analysis; Analyzer answers the question despite the issue; Fixer records separate follow-up without claiming missing data was recovered; Finalizer creates a final output only after the controller records approval. It uses deterministic Python commands and the same Simon Says controller as the other examples. It is not Magic simulation, scientific assessment, or a model-agent test.

Copy the directory to a new Git repository and commit it. Write `How many observations were recorded?` to a separate context file, then use `run ... --context /path/to/context.txt`. The Investigator compares this frozen original task with the declared input and fails closed if they differ. All action packets receive the same context.

The Fixer stage sets `approval_required = true`. After its independent review, the controller stops at `awaiting_approval`, before Finalizer starts. Check `status` and use the exact pending approval ID, accepted commit and snapshot digest with `approve --expected-approval ... --expected-commit ... --expected-digest ...` to continue. The CLI integration test simulates an operator approval in a temporary repository; it is not permission to publish, delete data, or perform external actions. To model a workflow without this gate, an author may remove `approval_required` before starting a new run.

The separate deterministic Reviewer invocation recomputes expected fixture values using the same Python helper. This demonstrates protocol and role handoff; it is not independent domain judgment. The fixture never deletes external data.
