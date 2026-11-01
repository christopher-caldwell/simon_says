# Implementation status

This is a partial implementation, not a v0 release. The executable provides `validate`, `freeze`, and `inspect` only. The example under `examples/basic` is a validator fixture; it is not a runnable workflow.

## Implemented and tested

- Strict TOML deserialization, version 1, bounded role/stage references, linear terminating and reachable pass routes, path checks, authoritative file loading, and JSON Schema compilation (external schema references rejected).
- A self-contained content snapshot and digest, exclusive snapshot creation, and verification of its internal digest. The digest is not independently anchored and does not resist intentional modification of the snapshot and its digest together.
- Pure in-memory routing logic for action identity, responses, review correction limits, and a bounded Unblocker detour. This code assumes a caller has already verified Git and outputs; no such controller exists.

## Missing

There is no durable run state, action invocation, provider integration, action log, Git worktree or checkpoint handling, artifact validation, autonomous run, crash handling, resume, reset, or executable domain example. No actual workflow has been run. Snapshot verification does not revalidate template semantics or establish a trusted authority root. Provider args and effort are parsed but not validated against the Orchestrate adapter policies. Validate is a preliminary check, not execution approval. Path checks cannot protect against concurrent filesystem substitution.

Real Magic tools and inputs have not been identified or tested. Neither the example nor the routing tests prove real provider, simulation, or end-to-end behavior. Git cannot undo external network or database side effects; any future recovery implementation must make that boundary explicit.

## Reference characterization

Orchestrate `281af897fcba59a4ef9476c2ce7aa40ed025b6ed` uses gate-specific controller logic tightly coupled to engineering Build, while its `crates/core/src/provider.rs` provides a potentially reusable invocation seam. Its tests validate persisted running actions, exact action IDs, detached review checkouts and bounded Unblocker routing. The new repository is the appropriate home for the generic controller; migrating Build or transplanting its entire controller would create avoidable coupling. Shared provider code remains to be integrated safely.

Baseline commands run before changes:

- Orchestrate `cargo test --workspace`: failed three CLI architecture tests concerning skill directories; 22 CLI unit tests passed before those failures.
- Orchestrate `cargo test -p orchestrate-build -p orchestrate-core`: passed 48 tests.
- Control Tower `cargo test --workspace`: one run passed; an independent concurrent run failed the SSE timeout test `web::tests::sse_shows_mutation_output_while_the_later_verifier_is_running`, and its isolated rerun also timed out. This suggests timing sensitivity; no cause was established.

The reference checkouts remained clean. No production system or provider was exercised.
