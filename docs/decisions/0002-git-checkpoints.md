# ADR-0002: Keep Git checkpoints in v0

**Status:** direction agreed, 2026-11-01.

## Context

The initial proposal questioned whether non-engineering tasks require a new artifact checkpoint substrate. The user explicitly prefers continuing to use Git, and the motivating simulation is in a Git repository.

## Decision

Use Git commits as the source of exact candidate and accepted stage snapshots for v0. The runner validates commit identity, clean worktree, ancestry, and declared output evidence. Git's familiar review/reset behavior is preferable to inventing another checkpoint system before proving need.

## Consequences

Explicitly model the difference between a mechanically valid candidate commit and a Reviewer-accepted stage. Design separate handling for execution logs not intended as product files. No automatic rollback of external side effects is implied. Worktree/branch protection remains an implementation decision.

## Detail

See [Git checkpoint design](../design/git-checkpoints.md).
