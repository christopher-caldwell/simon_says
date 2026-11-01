# V0 acceptance matrix

**Status:** proposed release gate. Automated tests should provide explicit source/log evidence rather than broad “verified” claims.

## Template and authority

| Case | Required result |
| --- | --- |
| Valid model-authored two-stage template | Loads, validates, snapshots, and executes without Rust changes |
| Unknown stage/role/transition | Prelaunch validation failure |
| Missing instruction/schema/profile | Prelaunch validation failure |
| Unreachable terminal or unsupported cycle | Prelaunch validation failure |
| Invalid or escaping file path | Prelaunch validation failure |
| Run directory inside source Git common directory | Reject before creating the run, even when the Git directory is separate from the source checkout |
| Live template modified after initialization | Running instance still uses frozen snapshot or refuses altered authority; never silently switches |
| Original run context modified or removed after initialization | All roles and retries receive the same frozen original task; the digest changes if stored context changes |
| Approval required after review | Stop for explicit approval bound to a unique review ID, accepted commit and frozen digest; no later stage runs first |
| Approval verification interrupted | Persist uncertain checking state; refuse replay until explicit recovery |
| Declared JSON Schema violated | Mechanical gate fails; no Reviewer pass/next stage |
| Protected prior-stage input altered | Deny direct action writes and reject changed blob or mode before review; ordinary inputs remain editable unless protected |

## Execution

| Case | Required result |
| --- | --- |
| Two-stage happy path | One launch executes all gates in order and records completion |
| Command Worker | Produces envelope and candidate Git commit satisfying same protocol |
| Agent Worker | Uses exact configured adapter/model/effort/profile and records receipt |
| Reviewer starts | Inspects exact submitted commit in clean detached review environment |
| Reviewer pass | Accepts stage and follows declared `on_pass` or the reviewed declared route label |
| Reported issue dispatches an authored role | A reviewed issue selects a frozen route to the configured repair role; the accepted repair can route back to verification without Rust domain logic |
| Conditional route invalid or missing | Fail closed; never dispatch an undeclared destination |
| Workflow transition budget exhausted | Stop durably before dispatching additional work; reset cannot extend the limit |
| Routed approval | Persist the reviewed route through restart and reject altered route or review evidence |
| Reviewer requests changes | Same-stage Worker gets original contract, exact feedback, and candidate lineage |
| Reviewer repeats changes past configured limit | Durable bounded stop with inspectable evidence |
| Reviewer wrong inspected commit | Fail closed; no stage advancement |
| Invalid action ID/outcome/JSON | Fail closed with reset-required or equivalent mechanically safe stop |
| Wrong Git HEAD, non-descendant commit, dirty candidate | Fail closed and retain last safe checkpoint |
| Worker leaves an ignored file in its managed checkout | Reject the candidate before review; preserve the file for explicit reconciliation and never affect ignored files in the source checkout |
| Required output omitted | Fail closed before review or transition |

## Recovery

| Case | Required result |
| --- | --- |
| Worker reports valid `blocked` | Unblocker receives originating gate and full feedback |
| Unblocker returns `retry` | Original gate is requeued using correct Git checkpoint and preserved context |
| Unblocker returns `blocked` | Clean durable stop |
| Second semantic block in same bounded attempt | Stop; no recursive Unblocker loop |
| Provider process fails | Recorded stop; no guessed replay or false completion |
| Kill process after persistent `running` | Fresh process detects uncertain work and requires explicit recovery |
| Resume valid ready state | Starts correct next action without consulting chat history |
| Reset | Verifies controlled worktree and checkpoint before destructive action; refuses unsafe checkout |
| Unknown external side effect during crash | Does not claim rollback or automatically replay without operator decision |

## Regression and proof of generality

- Existing Orchestrate Build tests and its Work/Review/Unblock checkpoints remain passing.
- Magic simulation runs **using its real tools and inputs** and one intentionally failing review/recovery case.
- A second non-Magic workflow runs through the same crate and CLI with different profiles and contracts, without source code changes.
- Status and logs report what actually occurred: action ID, stage, role, provider config, candidate and accepted commits, feedback, stop reason, and recovery instructions.
- Documentation labels any untested provider/sandbox combination, external side effects, and non-implemented feature honestly.

## Recommended test tiers

1. Pure parser/state routing tests (no subprocess): high coverage of legal/illegal transitions.
2. Fake role invoker with controlled Git fixture: deterministic full controller tests.
3. Real command worker + Git integration tests, including kill/restart.
4. Bounded provider smoke tests with recorded version/limits.
5. Real Magic workflow and independent second-domain acceptance.

V0 is **not** accepted merely because an agent reports that a test passed; tests must have process artifacts or verifiable fixture results.
