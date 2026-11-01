# Simon Says

Simon Says is an early local, sequential workflow runner. A TOML definition assigns Worker, Reviewer and Unblocker roles to stages; the Rust controller validates and snapshots the definition, executes actions, checks Git candidates and declared artifacts, and routes the results. This is **not yet an accepted v0 release**: see [implementation status](docs/implementation/status.md) for evidence and open reliability limitations.

```sh
cargo test
cargo run -- validate examples/data-report/workflow.toml
cargo run -- run /path/to/workflow.toml --project /path/to/clean-git-project --run-dir /path/outside/project/new-run
cargo run -- status /path/outside/project/new-run
cargo run -- resume /path/outside/project/new-run
```

`run` requires a new run directory whose parent exists and a clean Git project. It continues until completion, a bounded stop, or an error. The project checkout is left alone; a detached worktree is created under the run directory. Each action has a packet, invocation, process receipt, stdout, stderr, and result or failure under `actions/`; detached review checkouts are retained under `reviews/`. `status` prints durable JSON including candidate and accepted commits. An interrupted or failed action stays `running` and `resume` refuses to replay it.

Recovery is explicit. First confirm the original action and its descendants have exited (the process receipt is diagnostic only: it may be absent after a crash and PIDs can be reused) and reconcile any external effects. Then use:

```sh
cargo run -- reset /path/to/run --acknowledge-external-effects
cargo run -- resume /path/to/run
```

Reset verifies the recorded worktree and checkpoint, discards tracked changes only there, and refuses untracked or ignored files. It never runs `git clean`. Git reset does not undo network calls, database writes, or other external effects. A correction-limit reset requeues the Worker and starts a new bounded correction attempt. A failed initialization cannot be resumed; inspect its run directory and worktree registration before manual cleanup.

See [authoring guide](docs/implementation/authoring-guide.md), the [data report](examples/data-report/README.md), and [synthetic opening-hand](examples/synthetic-opening-hand/README.md) examples. Neither synthetic simulation nor structural schema validation proves domain correctness.

**Trust boundary:** commands and agent processes run with the invoking user's privileges. Git worktrees are isolation for checkout contents, not a security sandbox. A malicious same-user action can access the run directory, Git object store and external resources; a coordinated change to both state and snapshot across a crash can evade the current digest check. The controller detects and restores many live mutations, but cannot enforce frozen authority against adversarial same-user processes. Only run trusted workflows in trusted repositories. Configured Git checkout filters also execute with user privileges. Provider arguments are trusted configuration, not a safety policy.
