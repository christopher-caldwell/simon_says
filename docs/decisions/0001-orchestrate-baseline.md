# ADR-0001: Use Orchestrate as the execution baseline

**Status:** direction agreed, 2026-11-01.

## Context

The desired feature is a configurable, persistent AI-task state machine. Dagu was already evaluated extensively, and Control Tower has been designed as the preferred local staged workbench. Its reversible stage model is useful but lacks native Worker/Reviewer/Unblocker recovery. Orchestrate already exhibits the required model with durable state, native agents, feedback and Git checks.

## Decision

Use Orchestrate's Build execution and recovery patterns as the base. Generalize the workflow definition so multiple domains can use stable Rust code with model-defined stages and profiles. Do not treat a generic DAG or script scheduler as the starting point.

## Consequences

Preserve tested reliability mechanisms and the ability for a Reviewer to decide domain outcomes; avoid hardcoding engineering semantics into the generic runtime. Existing Build remains a separate compatibility concern during implementation.

## Evidence

[Orchestrate architecture](https://github.com/christopher-caldwell/ochestration/blob/main/docs/architecture.md) and [Build reliability lessons](https://github.com/christopher-caldwell/ochestration/blob/main/docs/evaluations/build-reliability/lessons.md).
