# Implementation status

This is a working early runtime, **not an accepted v0 release**. See the README for commands, recovery and the same-user trust boundary.

## Implemented

- Strict versioned TOML loading, named acyclic pass routes, reference and path checks, frozen instructions/profiles/schemas, and draft-07 JSON Schema compilation and artifact validation.
- A locked durable JSON run record and per-action packet, exact invocation, transport logs, result/failure; `preparing` is persisted before action-specific Git checks and checkout, and `running` before action invocation. Interrupted or failed actions require explicit reset rather than automatic replay.
- Detached worker and review worktrees, full Git object identity and ancestry checks, tracked regular-file artifact checks, separate candidate and accepted commits, and a narrowly verified reset that refuses untracked or ignored files.
- Command, Codex and Claude invocation plans, action envelopes, Worker/Reviewer correction loops, bounded Unblocker routing, and automatic continuation through stages.
- Two executable examples with the same engine and subprocess/Git tests, including corrections, Worker and Reviewer blocks, invalid schema and identity, process failure, and fresh-process interruption.

## Limitations and remaining acceptance work

- macOS action processes have a direct-write guard protecting run authority except the active checkout and per-action scratch; execution fails closed on other platforms. Same-user command and provider processes are trusted, and can still use IPC or modify shared Git objects/refs or external resources. Direct write attempts are blocked even if the controller dies, and live state/snapshot changes are detected after a returning action and restored where possible; same-user IPC can still defeat the guard. A stronger OS privilege boundary is necessary to enforce frozen authority against hostile actions. Git checkout filters can also run before an action is marked running; hooks are disabled for managed Git commands.
- A blocked Worker with dirty tracked changes stops for explicit reset. Unblocker actions run in a read-only-intent checkout; they cannot commit repository repairs. Git cannot undo external effects. Operators must confirm action processes and descendants have exited before reset; escaped process groups cannot be verified.
- Reviewer/Unblocker worktrees are retained by default; an explicit completed-run cleanup verifies them before removal. Interrupted review preparation can leave an orphan review directory with no recorded ownership; it is retained and requires manual reconciliation. Interrupted initialization records source identity and baseline; `recover-init` resumes a reservation or explicitly adopts a verified checkout. Legacy null-intent or unsafe partial registrations require manual reconciliation. Older runs without recorded checkout/admin/source/common filesystem identities fail closed for checkout operations and reset; old review packets without ownership records cannot be cleaned automatically. Actions have bounded role-configurable timeouts; an interactive cancellation command is not implemented.
- Schema support is draft-07 without references; selected later-draft keywords are conservatively rejected. JSON Schema verifies structure, not domain correctness.
- Checkpoint validation rejects submodules and non-UTF-8 tracked paths. It hashes tracked content using Git clean filters; those trusted repository filters can hang or cause external effects before an agent starts.
- Codex has been smoke-tested as Reviewer in both the local data workflow and a real native Magic diagnostic in one local configuration. Claude has also passed a separate guarded structured-data Reviewer smoke. Other models and permission/sandbox combinations have not been tested live. Profile settings do not provide a security boundary.
- The synthetic opening-hand example is not the real Magic simulator. The real simulator also ran through the controller in isolated one-game diagnostics, with induced block, retry and correction; see [evidence](evidence.md). A single diagnostic game cannot establish simulation quality.

## Reference baseline

Orchestrate `281af897fcba59a4ef9476c2ce7aa40ed025b6ed` has an engineering-specific Build controller and a reusable provider seam; this repository adapts provider patterns without modifying Build. `cargo test -p orchestrate-build -p orchestrate-core` passed 48 tests. `cargo test --workspace` failed three CLI architecture tests concerning skill directories. Control Tower `cargo test --workspace` passed once, while a concurrent run and isolated rerun timed out in `web::tests::sse_shows_mutation_output_while_the_later_verifier_is_running`; the cause was not established. Reference checkouts were left clean.
