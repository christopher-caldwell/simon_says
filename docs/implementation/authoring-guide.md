# Authoring a workflow

Use `examples/data-report/workflow.toml` as an executable starting point. An optional top-level `purpose` describes the reusable workflow and is included in every action packet. The workflow and its instruction, profile and schema paths are relative to the template directory. Inputs and outputs are paths relative to the project Git root. Names use ASCII letters, digits, `_` and `-`. All stages must be reachable from `start`, with at least one route to `complete`. Define named roles with their profiles and assign a Worker, Reviewer and Unblocker to each stage. Set `max_review_cycles` from 1 to 100. A simple workflow uses acyclic `on_pass` destinations; a workflow with named routes or cycles must also set `max_stage_transitions` from 1 to 1000.

Each role may set `timeout_seconds` from 1 to 86400 (default 1800); a timeout stops its process group and leaves an uncertain action requiring explicit recovery. A `command` role provides an argv array and runs directly, without a shell. `agent` roles support `codex` and `claude`, an optional model, effort and native args, and a required frozen profile. Model and native permission options are trusted local configuration. Run `simon-says validate <template>` before running. The controller checks inputs and outputs are regular files tracked at the recorded commit. A stage may name one JSON output schema when it has exactly one output. Schema validation uses draft-07; absent `$schema` means draft-07. References and selected newer-draft keywords are rejected. The conservative keyword check also rejects those words as property names. Unknown schema extensions may be ignored by the validator; use draft-07 assertions and test the contract. Arbitrary semantic claims belong in review instructions or a deterministic reviewer.

A reviewed stage can choose a workflow-defined route. For example, to dispatch a repair role only when an independently reviewed report finds an issue, and then rerun the original stage, use:

```toml
max_stage_transitions = 4

[stages.simulate]
# Define worker, reviewer, unblocker, instructions, inputs and outputs here.
on_pass = "complete"
max_review_cycles = 2

[stages.simulate.routes]
issues = "repair"
clean = "complete"

[stages.repair]
# Assign the authored repair profile as worker, with its own reviewer and outputs.
on_pass = "simulate"
max_review_cycles = 2
```

For a stage with `routes`, `on_pass` remains required and must match a declared destination. A passing Reviewer supplies a `route` label from that stage's declared map. The controller binds the decision to the inspected candidate, resolves the destination itself, and rejects missing, unknown or unauthorized labels. Workers, Unblockers and non-passing Reviewers cannot select routes. `max_stage_transitions` counts accepted stage transitions including completion; the controller stops before starting a stage if there is no remaining transition budget to complete. Reset cannot extend this limit. Review corrections and Unblocker retries have their existing separate bounds.

Pass an issue report as a tracked, schema-validated output of the observation stage and declare it as an input to the repair stage. The repair action's `last_transition` packet field identifies the prior reviewed commit, action, stage and selected route; stage feedback is not carried into unrelated stages. Authors must define the issue contract and review criteria; the controller does not interpret domain errors. A workflow uses one isolated Git project: changes to separate source repositories or external side effects require explicit workflow-owned integration and remain subject to the trust boundary. See `examples/dynamic-repair` for the executable controller test; its simulator and repair are deterministic fixtures, not the Magic engine.

On the supported macOS platform, the direct-write guard blocks actions from writing run authority. Some setuid tools such as macOS `/bin/ps` cannot execute under the guard. Each command receives a writable private action scratch directory in `SIMON_SCRATCH` and the absolute JSON packet path in `SIMON_PACKET` and writes one JSON response to stdout. The packet includes action ID, stage, gate, frozen instructions/profile/schema, input and output paths, feedback, originating blocked gate, and checkpoint identities, declared reviewer route labels, and the preceding transition when present. Each response supplies `action_id`, `outcome`, nonempty `report`, optional `candidate_commit` and `inspected_commit` fields, and a `route` label when required by a routed reviewer pass. Valid outcomes:

| Gate | Outcomes | Required commit |
| --- | --- | --- |
| Worker | `complete`, `blocked` | `candidate_commit` on complete |
| Reviewer | `pass`, `changes_required`, `blocked` | `inspected_commit` on every response |
| Unblocker | `retry`, `blocked` | none |

A completing Worker commits outputs itself and leaves a clean detached checkout. A blocked Worker must leave HEAD and checkout unchanged; dirty partial work stops for explicit recovery instead of invoking the Unblocker. Reviewer and Unblocker run in separate detached checkouts and must leave them unchanged. The reviewer must inspect the exact candidate. A correction returns feedback and the candidate to the same stage's Worker. One Unblocker retry is allowed per blocked attempt; a second block stops. All subprocesses must wait for their children: surviving same-process-group descendants make an action uncertain. Escaped process groups cannot be reliably controlled.

The engine validates mechanics and routes the Reviewer's decision; it does not decide whether that judgment is correct. Input files must be committed in the baseline or created and accepted by a previous stage. Template instructions and schemas are frozen in the run snapshot, not reread from the original template during execution. See the README's trust boundary before running untrusted code.

Review checkouts must remain clean even when tools create caches. Configure such tools to write caches under `SIMON_SCRATCH` or disable them (for example, use `python3 -B` or `PYTHONDONTWRITEBYTECODE=1` for Python). A dirty review fails closed even if its verdict is otherwise valid.

To preserve the user’s original task and constraints across every handoff, pass `--context /path/to/task.txt` to `run`. Simon Says snapshots the nonempty UTF-8 file (maximum 32 KiB) before creating the run, binds it into the run digest, and sends the same `run_context` to Worker, Reviewer and Unblocker, including after restart or correction. The original file can then change without changing the run. Context is task data: it cannot alter the frozen routes, contracts or limits. It is retained in the run snapshot and action packets and sent to configured providers. Native-provider prompts are supplied through stdin and limited to 4 MiB total; run context remains limited to 32 KiB. The exact prompt is retained in the action record.

On current macOS, Claude Worker Bash sandboxing cannot normally nest inside the controller’s write guard. The controller probes this before invoking a Claude Worker and stops safely if nesting is unsupported; Claude Reviewer and Unblocker roles are not subject to this Worker-only preflight. Do not disable either sandbox implicitly to work around it.

For a workflow that must pause before publishing or retiring evidence, set `approval_required = true` on the stage whose reviewed output requires approval. The runner records the accepted candidate and stops at `awaiting_approval`. Inspect `status`, the candidate, and `simon-says inspect RUN_DIR/snapshot.json` (which includes the frozen original context), then explicitly run `simon-says approve RUN_DIR --expected-commit COMMIT --expected-digest DIGEST --expected-approval APPROVAL_ID [--note TEXT]`. The expected values must match the recorded accepted commit, frozen snapshot including run context, and unique pending approval ID shown by `status`. The ID binds approval to this specific Reviewer action, preventing a prior approval from being replayed for another stage even if its commit is unchanged. Approval is recorded in durable state, and the command continues the declared route until completion or the next stop. This is an explicit local-operator gate; it does not authenticate a remote user or approve other commits, workflows, or external side effects.

For the motivating Magic process, an author can define separate stages and profiles for Investigator, Supervisor, Analyzer, and Fixer: carry the original question as run context; have the Supervisor collect structural evidence and backlog issues without deciding the outcome; hand the Analyzer the frozen question and accepted evidence; and place an approval gate before any later finalization or retirement stage. Fixing backlog issues can be a separate stage or workflow without silently replacing the analysis. The engine starts fresh native provider invocations. Codex exec emits distinct recorded thread IDs; whether those sessions appear as the separate Desktop chats you expect is not verified. The engine does not schedule spaced future checks. A workflow requiring those capabilities needs an external integration, and an approval gate alone does not authorize deleting arbitrary data.

Approval verification enters a durable `approval_checking` state before Git checks, which can invoke trusted clean filters. If that check fails or is interrupted, the user must reconcile external effects and explicitly reset; reset returns the run to `awaiting_approval`, requiring a fresh explicit approval. A failed approval does not authorize continuation.

Declare any project-resident authority that actions execute or rely on—such as a deterministic reviewer, helper, or simulator configuration—in top-level `frozen_project_files = ["review.py", "config.json"]`. Every entry must be a regular tracked file in the starting commit. Before invoking a role and before accepting a Worker candidate, the controller compares each file's Git blob identity and mode against that baseline; the macOS action guard also denies direct changes to those files and protects their ancestor directories from renaming. A Worker cannot replace or relax those declared checks during the run. The engine cannot infer every indirect dependency of a script; the workflow author must declare the authoritative project files. Template instructions, profiles, and schemas are separately captured in the snapshot.
