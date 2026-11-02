# Autonomy gaps observed during the Magic study

Status as of November 1, 2026: **open**. The goal is to start an authored
workflow and let Simon Says dispatch its configured roles, route reported
issues, and continue to a result or a specific durable stop without an
operator designing and launching additional workflows. A real Magic study has
not yet demonstrated that end-to-end behavior. This document separates
controller gaps from mistakes in the particular workflow.

## Controller and product gaps

1. **Malformed responses require operator recovery.** A Luna Worker completed
   and committed its correction but added fields to the fixed response
   envelope. Simon Says recorded `invalid action response; reset required`
   instead of giving the role a bounded opportunity to correct the response.
   `adopt-worker` currently handles a different, narrowly defined stop; it
   cannot adopt this result. The operator created another run from the
   preserved commit. The envelope and action identity must remain enforced;
   a recovery design must not blindly rerun an uncertain action or silently
   treat an invalid response as valid.

2. **A malformed blocked response bypassed the workflow's recovery route.**
   The collection command reported `blocked` with an `inspected_commit`, which
   is illegal for a Worker. The controller recorded `blocked worker cannot
   report commits` and stopped with the action still marked running. Its
   configured Unblocker was therefore never invoked. This behavior enforces
   the present contract, but it demonstrates that a predictable formatting
   mistake can require a human or supervising assistant to intervene before
   the authored recovery route can run. The controller must not assume an
   invalid envelope establishes what the action did.

3. **The existing recovery primitives do not cover this completed-action
   failure.** The collection command had finished, and its journal and native
   records showed prepared assignments, but the controller lacked an explicit
   way to reconcile that outcome and route it to a configured repair role.
   Instead the operator authored two separate Simon Says repair workflows and
   started them manually. A workflow-defined recovery handoff should be able
   to carry action identity, immutable evidence, and an explicitly reconciled
   checkpoint to an authored role, with bounded attempts and an honest stop
   when the evidence is insufficient. This does not imply automatic replay or
   a domain-specific Magic repair in Rust.

4. **Autonomous continuation after domain recovery is not demonstrated.** The
   second repair workflow produced a reviewed, read-only reconciliation of
   the 20 existing runs, but it did not implement or verify a safe launch from
   those runs. A successful repair workflow therefore did not automatically
   return control to the interrupted study. The original workflow's `stop`
   Unblocker was intentionally configured to stop, so this is not evidence
   that an authored fixer route was selected incorrectly. It shows that the
   necessary authored recovery route and its controller-supported handoff have
   not yet been proven for this failure.

5. **Multi-scenario unattended operation remains unverified.** Separate
   workflows and sequential routing have been exercised, but no evidence from
   this study shows several user-defined research scenarios launched once,
   executing sequentially, and reporting completion or a durable blocker
   without an operator constructing intermediate workflows. The user-facing
   objective requires that proof, including failure and recovery cases.

## Magic workflow defects and limits

- The queue script removed `assigned` from its plan arms and later read the
  removed field, causing the collection failure. It also emitted an invalid
  blocked envelope. These are workflow implementation defects, not reasons
  to add Magic-specific behavior to the controller.
- The initial no-game test checked counts but did not exercise the normal
  native preparation handoff. An independent reviewer also caught a wrong
  action-identity guard before collection. The repaired script has fake-data
  tests and an independent review; real game execution after the fix remains
  unverified.
- At inspection, the 20 CP3 runs held 1,000 pending trials and zero attempts.
  A separately reviewed reconciliation checked those records read-only. It
  did not prove exclusive ownership during launch, authorize replay, or
  reconcile the separate CP4 queue. No new study games or research findings
  resulted from these attempts.
- Claude's session limit stopped an earlier provider action. That is a provider
  availability limitation, not proof of a controller routing defect. The
  OpenAI retry used configured roles and reached collection.

## Evidence and follow-up

The local run directories are siblings of the `simon_says` checkout under
`implement-simon-says-autonomous-v0-development`:

- `magic-optimization-openai-run-v1/actions/action-2/` records the malformed
  completed Worker response and failure.
- `magic-optimization-openai-run-v2/actions/action-4/` records the invalid
  blocked response and collection journal. The journal records the prepared
  runs; the native database was checked for trial and attempt state.
- `magic-optimization-repair-run-v1/` records the reviewed script repair at
  commit `ba16a08179494ccb643e7af4b7f79cbb146b6a92`.
- `magic-optimization-recovery-run-v1/` records read-only reconciliation and
  review at commit `62552dd61d5f5ecc9ebe1b62159e2d530d0b3242`.

Next, reproduce both invalid-envelope stops with deterministic controller
fixtures; design and test a bounded, explicit reconciliation-to-authored-role
handoff that preserves action and checkpoint identity. Separately finish and
review the Magic workflow's reuse/launch path against fixtures, then prove a
real small run and sequential scenarios without manually creating intermediate
workflows. Keep failures visible and stop before additional paid actions when
the workflow is broken.
