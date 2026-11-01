# Documentation guide

This repository contains the design and a partial implementation of a generic workflow runner. The CLI currently validates and snapshots definitions; execution is not implemented. See [implementation status](implementation/status.md). Source-of-truth rules:

1. [Vision](vision.md) describes the problem and intended outcome.
2. [Scope](scope.md) contains the normative v0 requirements and exclusions.
3. [Decision records](decisions/README.md) record agreed direction; open choices are marked as proposals, not retroactive user decisions.
4. [Design](design/architecture.md) and related documents specify a candidate technical approach.
5. [Implementation plan](implementation/plan.md) defines a safe development sequence; [acceptance](implementation/acceptance.md) defines what must be demonstrated.
6. [Open questions](implementation/open-questions.md) tracks choices not resolved by these documents.

## Read in this order

Vision → Scope → Architecture → Template → Agents/contracts → Git → Recovery → Authoring → Implementation → Acceptance → Open questions → Example.

## Document map

- `design/`: proposed architecture, workflow definition, data and execution boundaries.
- `implementation/`: work breakdown, acceptance, and deliberate unresolved issues.
- `examples/`: illustrative domain applications; do **not** treat sketches as an implemented parser contract.
- `decisions/`: short records separating accepted choices from recommended defaults.
- `references/`: facts and limitations of the existing tools, grounded in source links.

## Editing conventions

- Distinguish **accepted direction**, **proposed default**, and **deferred** behavior explicitly.
- Preserve the accepted goals while challenging assumptions with tests.
- Keep model-facing instructions separate from machine-enforced rules.
- Update scope, template specification, acceptance tests, and examples together when changing a behavior.
- Do not modify Orchestrate Build while validating the generic runner unless a change is proved non-regressive and expressly included in the implementation plan.
- Never make an external side-effect rollback or concurrency guarantee without implementation evidence.
