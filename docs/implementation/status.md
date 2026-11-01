# Implementation status

This is a working early runtime, **not an accepted v0 release**. See the README for commands, recovery and the same-user trust boundary.

## Implemented

- Strict versioned TOML loading, named acyclic pass routes, reference and path checks, frozen instructions/profiles/schemas, and draft-07 JSON Schema compilation and artifact validation.
- A locked durable JSON run record and per-action packet, exact invocation, transport logs, result/failure; running is persisted before action invocation. Interrupted or failed actions require explicit reset rather than automatic replay.
- Detached worker and review worktrees, full Git object identity and ancestry checks, tracked regular-file artifact checks, separate candidate and accepted commits, and a narrowly verified reset that refuses untracked or ignored files.
- Command, Codex and Claude invocation plans, action envelopes, Worker/Reviewer correction loops, bounded Unblocker routing, and automatic continuation through stages.
- Two executable examples with the same engine and subprocess/Git tests, including corrections, Worker and Reviewer blocks, invalid schema and identity, process failure, and fresh-process interruption.

## Limitations and remaining acceptance work

- Same-user command and provider processes are trusted, not sandboxed by Simon Says. They can write the run directory and shared Git objects/refs or perform external effects. Live state and snapshot changes are detected after a returning action and restored where possible, but a process can alter both records and kill the controller before that check. A stronger OS privilege boundary is necessary to enforce frozen authority against hostile actions. Git checkout filters can also run before an action is marked running; hooks are disabled for managed Git commands.
- A blocked Worker with dirty tracked changes stops for explicit reset. Unblocker actions run in a read-only-intent checkout; they cannot commit repository repairs. Git cannot undo external effects. Operators must confirm action processes and descendants have exited before reset; escaped process groups cannot be verified.
- Reviewer/Unblocker worktrees are retained; there is no managed worktree cleanup command. Interrupted initialization requires inspection/manual cleanup. Action timeouts and cancellation are not implemented.
- Schema support is draft-07 without references; selected later-draft keywords are conservatively rejected. JSON Schema verifies structure, not domain correctness.
- Codex has been smoke-tested in one local configuration. Claude, other models and permission/sandbox combinations have not been tested live. Profile settings do not provide a security boundary.
- The synthetic opening-hand example is not the real Magic simulator. The real simulator also ran through the controller in an isolated one-game diagnostic, with induced block, retry and correction; see [evidence](evidence.md). A single diagnostic game cannot establish simulation quality.

## Reference baseline

Orchestrate `281af897fcba59a4ef9476c2ce7aa40ed025b6ed` has an engineering-specific Build controller and a reusable provider seam; this repository adapts provider patterns without modifying Build. `cargo test -p orchestrate-build -p orchestrate-core` passed 48 tests. `cargo test --workspace` failed three CLI architecture tests concerning skill directories. Control Tower `cargo test --workspace` passed once, while a concurrent run and isolated rerun timed out in `web::tests::sse_shows_mutation_output_while_the_later_verifier_is_running`; the cause was not established. Reference checkouts were left clean.

A local macOS `sandbox-exec` feasibility probe in disposable fixtures denied writes to run authority and the source checkout while allowing commits in a private clone. This is not integrated: provider cache/IPC requirements, nested sandboxes, platform portability, private object ownership and adversarial behavior need integration tests before it can support an authority guarantee. The existing linked worktree shares repository objects and is intentionally not described as a sandbox.
