# Isolated real-engine Fixer investigation

This user-requested exercise targets the `Count$LeftBattlefieldThisTurn Creature.YouCtrl` diagnostic observed in four retained Magic attempts. It clones the **recorded historical engine commit** into a new project, snapshots the actual Fixer profile, and starts the configured Fixer through Simon Says. It does not modify the original checkout, install a candidate runtime, run games, publish, or claim overall engine correctness. The candidate is a local checkpoint in the disposable clone. The frozen reviewer runs the focused Rust regression against the exact candidate in a fresh worktree.

The engine's recorded source includes an unused `forge` Git submodule. Simon Says v0 rejects submodules, so preparation removes only that gitlink in the new clone; it does not alter the source repo. A failed preparation can leave a new incomplete clone requiring inspection. The clone has full local Git history; the reference checkout can be dirty because the historical commit is read from Git objects, not its working tree. Keep isolated projects outside both sources.

```sh
python3 -B examples/magic-real-fix/prepare.py /absolute/manabrew /absolute/collection_plan /absolute/new/project
(cd /absolute/new/project && simon-says validate simon-workflow.toml)
DYLD_SHARED_REGION=avoid simon-says run /absolute/new/project/simon-workflow.toml --project /absolute/new/project --run-dir /absolute/new/run
simon-says status /absolute/new/run
```

Native provider authentication, Rust/Cargo dependencies and substantial time and space for isolated builds are required. The Fixer receives the existing source profile's configured model and effort. The task is bounded to source and tests under the engine crate, plus `repair.json`. The reviewer verifies the exact candidate and runs a nonempty integration-test target; it is a mechanical check, not independent judgment of all game rules.

## Recorded run

The run at `../magic-real-fix-run` used source commit `44f09e473213606382168f0b08442057823b9128`, source profile SHA-256 `39ac3579382c490c279bb73c1c308537353c9308dcdabf2eb33cd971a7466712`, and `gpt-6.1-terra/high`. In the isolated candidate `9e1307fd6ea721b9c7630e0bba7b62f742b1d005`, the two new focused tests failed against the historical source with the observed unrecognized-expression diagnostic and passed with the repair. The independent Reviewer compiled and passed both tests in its own action scratch. The Fixer also passed ten neighboring `zone_change_effects` tests with `RUST_MIN_STACK=16777216`; the same neighboring suite aborted with the default test-thread stack. The cause of the default-stack failure was not established. The candidate implements only the requested selector and inherits the move path's existing pre-replacement snapshot; replacement/continuous-effect coverage, archived card scripts, candidate runtime acceptance and end-to-end games remain unverified. See the candidate's `repair.json` for exact commands and limits.

The preparation was independently rerun from the local reference repositories, reproducing the original project tree `b615dba0a252040ea70d42f2155f07a03ef9f50f` and workflow digest `7213bd4b04142654990fdbdede6d30224d03e7f35e7105a1fcd35ef0903fdfae`. The single Fixer invocation reported 1,420,202 input tokens (1,379,328 cached) and 6,951 output tokens; monetary cost was not recorded.
