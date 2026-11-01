# ADR-0005: Implement the generic runtime in Simon Says

**Status:** implementation decision, 2026-11-01; not a new user decision.

The original proposed sibling crate assumed development inside Orchestrate. Simon Says now has its own repository. Inspection of Orchestrate at `281af897` found its Build controller coupled to engineering adoption, phases, audit and store semantics, while the provider adapter supplies reusable invocation patterns. Transplanting or refactoring the Build controller would risk existing behavior without demonstrating genericity.

The generic runtime therefore lives in the Simon Says repository. Its provider code adapts Orchestrate's invocation conventions; its workflow policy, persistence and isolated Git checkpoints are implemented independently. Orchestrate remains unchanged. This decision does not claim parity: the [implementation status](../implementation/status.md) records current verification and gaps, including native-provider coverage and the same-user trust boundary.
