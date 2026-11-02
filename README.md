# Simon Says

Simon Says is an early local, sequential workflow runner. A TOML definition assigns named roles to stages; the Rust controller validates and snapshots the definition, launches roles, checks their Git checkpoints and declared artifacts, and follows authored, bounded transitions. Reviewers and Unblockers are optional per stage. Authors define their own named role profiles; the controller does not contain a domain-specific Fixer. This is **not yet an accepted v0 release**: see [implementation status](docs/implementation/status.md) for evidence and open reliability limitations.

Its core jobs are to read the workflow, launch its configured roles, pass their outputs between stages, follow the workflow's declared routes, and preserve run state. A model or tool performs the work defined by its role; Simon does not decide domain-specific answers.

```sh
cargo test
cargo run -- validate examples/data-report/workflow.toml
cargo run -- run /path/to/workflow.toml --project /path/to/clean-git-project --run-dir /path/outside/project/new-run --context /path/to/original-task.txt
cargo run -- status /path/outside/project/new-run
cargo run -- resume /path/outside/project/new-run
```

`run` requires a new run directory outside the source checkout and its Git common directory, whose parent exists, and a clean Git project. `--context` is optional; when supplied, its original text is frozen and included in every role packet. It continues until completion, a bounded stop, or an error. The project checkout is left alone; a detached worktree is created under the run directory. Each action has a packet, invocation, process receipt, stdout, stderr, a writable scratch directory, and result or failure under `actions/`; detached review checkouts are retained under `reviews/`. `status` prints durable JSON including candidate and accepted commits and can be read while an action is running. Interrupted or failed preparation stays `preparing`, and an interrupted or failed invocation stays `running`; `resume` refuses to replay either.

Recovery is explicit. First confirm the original action and its descendants have exited (the process receipt is diagnostic only: it may be absent after a crash and PIDs can be reused) and reconcile any external effects. Then use:

```sh
cargo run -- reset /path/to/run --acknowledge-external-effects
cargo run -- resume /path/to/run
```

Reset verifies the recorded worktree and checkpoint, discards tracked changes only there, and refuses untracked or ignored files. It never runs `git clean`. Git reset does not undo network calls, database writes, or other external effects. A correction-limit reset requeues the Worker and starts a new bounded correction attempt. Reset cannot extend an exhausted workflow transition limit. For interrupted initialization, `recover-init <run-dir>` resumes a recorded reservation. If checkout creation may have started, it requires `--acknowledge-external-effects` and an explicit choice: `--adopt-existing` for a verified checkout or `--retry-create` when the target is absent and unregistered. It refuses unidentifiable targets and never deletes them. Older run records without filesystem ownership identities require manual reconciliation for checkout operations. Clean review worktrees from a completed run can be removed with `cleanup-reviews <run-dir> --acknowledge-processes-exited`; action evidence and the Worker checkout are retained.

Start with the [three-model CSV workflow](examples/csv-questions/README.md) or the [model-dispatched repair loop](examples/model-dispatch/README.md). The [authoring guide](docs/implementation/authoring-guide.md) describes optional review, recovery and other examples. Neither a fixture nor schema validation proves domain correctness.

**Execution:** Simon Says launches actions with the invoking user’s permissions and does not sandbox them. Provider-native settings remain configurable. The controller validates frozen authority, candidate commits and declared artifacts when actions return; those checks are not a security boundary against hostile processes.
