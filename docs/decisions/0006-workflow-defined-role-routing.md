# ADR-0006: Workflow-defined roles and bounded reviewed routes

Status: User direction agreed; routing details are an implementation decision.

## Context and direction

The user clarified that named roles and their profiles belong to each authored workflow. Simon Says must dispatch them according to that workflow, rather than hard-coding a domain-specific Fixer or an Orchestrate-style fixed role mapping. One motivating workflow observes simulation errors, routes them to a configured repair role, reviews its work, and verifies the result. Earlier acyclic-only v0 guidance was an implementation simplification; it cannot express this required flow.

## Implementation decision

A stage may declare named route labels and destinations, and a Reviewer selects a label in the fixed response envelope after inspecting the exact candidate. The controller resolves only a frozen declared label, records the transition and enforces a finite workflow-wide transition limit. A stage can return to a previous stage within that limit. Legacy linear definitions remain valid. A repair role is a normal named Worker with its own profile and Reviewer; the controller has no Magic-specific error classification or Fixer implementation. A workflow carries issue evidence through tracked, reviewed artifacts.

This initial design chooses Reviewer-selected labels to preserve Orchestrate's review gate and keep semantic judgment with the agents. Other selection mechanisms are not implied. These bounds cannot undo external effects, and actions run with the invoking user’s permissions. A production Magic workflow still requires its own verified inputs, executable simulation and recovery semantics.


## Follow-up: workflow-selected route owner

The reviewer-only mechanism was an implementation choice, not a user decision.
A stage can now opt into `route_selector = "worker"`: its Worker proposes a
frozen declared label as part of its completed candidate. The Reviewer sees
that proposal and must pass the exact candidate before the controller follows
it. The default remains reviewer selection for existing definitions. This lets
an authored role choose a handoff without introducing a built-in supervisor or
fixer; the workflow still determines the available destinations and bounds.
