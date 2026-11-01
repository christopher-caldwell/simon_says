# Vision and motivation

## The desired experience

A user describes an objective and existing resources to a capable model: “When I propose a Magic deck, run the existing simulation, validate its results, fix routine failures, review the output, and produce analysis.” The model designs a **reusable workflow template**, including the required stages, agent profiles, inputs, outputs, acceptance criteria, and failure handling. The user can inspect and approve that definition once.

Later runs take new input and use the **same deterministic runtime**. They should not require the user to supervise each stage or a model to remember its own sequencing instructions. The workflow stays at the precise recorded state when something fails; it either follows a permitted recovery route or stops with actionable evidence.

Other examples include ETL verification, document analysis, recurring experiments, and software engineering workflows. They should share engine code while supplying different definitions and agent instructions.

## Observed pain

The motivating Magic deck simulation pipeline already exists, including task roles and tools, but its coordinating AI sometimes:

- Gets stuck when a tool, script, or environment is not ready.
- Doesn't follow every ordered instruction or required validation.
- Spends expensive high-reasoning model time on mundane execution.
- Loses track of which artifact or stage is authoritative.
- Requires human attention to identify whether to retry, repair, or continue.

The desired response is not a better prompt; it is to **remove workflow authority from the prompt**.

## Why Orchestrate is the baseline

Existing Orchestrate Build has a persistent Rust controller, native provider adapters, immutable action packets, schema-checked results, Git invariants, Worker ↔ Reviewer correction, and one bounded Unblock detour. The controller transports evidence and outcomes rather than making the engineering judgment of the Reviewer. Those are the valuable elements to preserve.

Its limitation is specificity: fixed `Work`, `Review`, `Audit`, and `Unblock` gates; engineering-specific reconciliation and adoption artifacts; mandatory source-commit behavior; and fixed phase treatment. The proposed system replaces **the workflow policy**, not the reliability machinery.

## North-star statement

> A model may design the workflow, and specialized agents may execute and judge its tasks, but only the deterministic controller can advance or change the durable workflow state.

## Success looks like

1. An authoring model creates a valid template with no Rust edits.
2. A low-cost worker performs routine work; more capable agents are assigned only where appropriate.
3. Inputs, outputs, feedback, and checkpoints can be inspected after every step.
4. A failed review returns actionable feedback to the correct worker without restarting everything.
5. A blocker invokes the configured Unblocker once and either retries safely or stops cleanly.
6. Two unrelated workloads use the same implementation with different templates.
7. Existing Orchestrate Build remains reliable throughout incremental development.
