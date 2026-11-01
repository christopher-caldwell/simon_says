# Git checkpoints and artifact identity

**Accepted design direction:** Git remains the v0 checkpoint mechanism. This is a deliberate choice, not a temporary workaround.

## Why Git fits

Orchestrate already validates commit ancestry, clean source state, exact `HEAD`, and detached review worktrees. The Magic simulation exists in a repository. Storing generated outputs in versioned files provides a clear source/candidate/evidence record and supports deterministic recovery from many failed stages.

## Separate three concepts

- **Run baseline:** the starting commit before any stage runs.
- **Current candidate:** a mechanically validated Worker-produced commit, which may be pending Reviewer acceptance.
- **Accepted stage checkpoint:** the commit whose corresponding stage review/checks passed and from which the next stage is authorized.

The existing Build controller's `checkpoint_commit` advances after a valid Worker completion, even while review is pending. The new runtime should preserve that reliable mechanic but make candidate/accepted status *explicit* in its state model or clearly derive it from the pending gate. Avoid claiming “approved” merely because a commit exists.

## Proposed v0 sequence

1. At initialization require a clean Git-visible starting checkout. Record baseline commit, chosen run worktree/branch identity, and frozen template digest.
2. Worker runs within a controlled run worktree. Completed workers report a candidate commit descended from the valid starting commit and equal to the run worktree's `HEAD`, with no uncommitted work.
3. Validate declared output files at **that exact commit**, not an arbitrary file on the parent checkout. Where possible, validate changed paths and required schemas.
4. Reviewer reads the exact candidate in a disposable detached worktree and cannot alter the artifact being assessed.
5. Reviewer `pass`: record stage accepted checkpoint and route to the next stage.
6. Reviewer `changes_required`: retain candidate and feedback, then route same-stage Worker for a descendant corrective commit.
7. Blocked or uncertain actions use documented [recovery semantics](recovery.md). No silent rewind of unrelated work.

## Worktree safety

**Proposed safe default:** use a dedicated run worktree/branch to prevent destructive resets in the user's ordinary working tree. Verify a detached review's clean state before/after invocation. Never `git reset --hard` or `git clean -fd` an arbitrary checkout passed without guarding ownership, run identity, and checkpoint ancestry.

This differs from some current Orchestrate Build details; the implementation team must explicitly decide whether to reuse the existing checkout policy for the first spike or introduce run worktrees immediately. Either approach must make reset consequences visible and be proven against real repos.

## Outputs and large files

In v0, declared artifacts are normal repository files and their exact content can be derived from the candidate commit. Do not silently stage secrets, huge binary caches, or intermediate model transcripts as product artifacts. Keep invocation logs and runtime metadata outside the product working tree; track references/digests in durable run state.

Define a path policy for ignored outputs (`.gitignore`), LFS assets and symlinks. Preferred v0: fail closed when a required output is not Git-visible in the candidate. This is a deliberate constraint to keep evidence reproducible.

## Git's limit

Git does **not** roll back requests to external APIs, outgoing mail, SQL mutations, invoices, network-side effects, or arbitrary subprocess effects. A commit provides an exact filesystem checkpoint, not a transaction spanning the world. Mark templates that touch external systems as requiring separate retry/compensation analysis. For v0, guaranteed automatic replay is limited to Git-contained effects; uncertain external effects stop for human-directed recovery.
