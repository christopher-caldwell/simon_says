# Simon Says

Simon Says is an early local, sequential workflow runner. A TOML definition assigns Worker, Reviewer and Unblocker roles to stages; the Rust controller validates and snapshots the definition, executes actions, checks Git candidates and declared artifacts, and routes reviewed results through authored, bounded transitions. Authors define their own named role profiles; the controller does not contain a domain-specific Fixer. This is **not yet an accepted v0 release**: see [implementation status](docs/implementation/status.md) for evidence and open reliability limitations.

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

See [authoring guide](docs/implementation/authoring-guide.md), the [data report](examples/data-report/README.md), [synthetic opening-hand](examples/synthetic-opening-hand/README.md), and [role handoff](examples/role-handoff/README.md), [dynamic repair](examples/dynamic-repair/README.md), [structured-data dispatch](examples/data-dispatch/README.md), and [isolated Magic roles](examples/magic-roles/README.md) examples. Neither synthetic simulation nor structural schema validation proves domain correctness.

**Execution:** Simon Says launches actions with the invoking user’s permissions and does not sandbox them. Provider-native settings remain configurable. The controller validates frozen authority, candidate commits and declared artifacts when actions return; those checks are not a security boundary against hostile processes.
