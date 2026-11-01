# Isolated modal action-offer repair

This fixture investigates **one observed warning**, not all native warnings. In the retained `magic-dynamic-game-run-v3` corrected batch, attempt `90042c23f1d24006acb51b46d787aad8` logged `Card play failed` at trace sequence 831 after the bot selected the offered Origin of Metalbending. A separate same-seed diagnostic replay at `../magic-warning-replay-origin-v1` showed no artifact/enchantment on any battlefield at that point; the same card was later cast when valid artifact targets existed. This is not an independent sample or evidence about deck performance.

A focused control uses the **actual frozen card archive** and the engine's real `get_playable_cards` implementation. At the recorded source candidate `97bfa479d17dcd494cb9196805f5650d92072d96`, the no-legal-mode Origin control failed while the legal-artifact Origin and a Boros Charm legal-alternative control passed. Its log is `../magic-warning-replay-origin-v1/baseline-tests.log`. This supports a specific action-offer/cast-time inconsistency; it does not prove that every `Card play failed` warning has that cause. Separately observed explicit target cancellations and target-invalid fizzles are not adjudicated as errors or harmless by this fixture.

`prepare.py` creates a new isolated clone, injects the frozen test module and role instructions, and uses the configured Fixer profile from the real Magic role source. It never modifies the engine or Magic reference checkouts. Its source repository argument must contain the recorded local candidate commit (for example, the earlier `magic-dynamic-game-project-v3`). It requires the exact archive SHA-256 `9fede9dbee4caed9e3774391215e040c23e142c0cc2e382b1ceeecb12ea2859b`; the Reviewer rechecks that identity. Cargo dependencies must be cached for offline compilation.

```sh
python3 -B examples/magic-warning-offer/prepare.py /absolute/local/engine-repo /absolute/collection-plan /absolute/new-project
SIMON_MAGIC_ARCHIVE=/absolute/path/to/cardset.rkyv \
  simon-says run /absolute/new-project/warning-workflow.toml --project /absolute/new-project --run-dir /absolute/new-run
simon-says status /absolute/new-run
```

The preparer adapts the role wrapper to identify `simon-warning/fix.md` explicitly; with the same source/profile, every prepared Git blob other than that wrapper matched the earlier live baseline. The frozen Reviewer runs three tests against the exact candidate in a separate review worktree and private target directory. The archive, original game evidence and all source reference checkouts remain untouched. No runtime is installed, and these source tests are not acceptance of full simulator behavior. The candidate and other limitations must be inspected before any deployment or use in a study.

## Recorded isolated run

On 2026-11-01, `../magic-warning-fix-project` was prepared at baseline `c1e5e86a099d891e6102489e5236e35df9f2f7b2`, with a frozen workflow digest `fa09efba8a8564b02bb74044be2503d732c3c72f80a71fe30a8a2096e7e49133`. The baseline regression returned one failure and two passes. One `../simon-warning-pinned run ...` dispatched the configured `gpt-6.1-terra/high` Fixer in Codex thread `01a198eb-0bd8-7481-973d-1132b7ee97ab`, then separately reviewed candidate `39c66e3f6e1676083c639e41e92388f244259446`. The independent Reviewer passed all three frozen real-archive tests; its full log is `../magic-warning-fix-run/actions/action-1/scratch/review-tests.log`. The Fixer also reported its own additional modal playability test and original phyrexian test passing. Model usage was 2,134,241 input tokens (including 2,062,464 cached) and 6,756 output tokens; monetary cost was not recorded.

The resulting candidate checks compatible ordinary modal spell options using the existing cast-time check, and deliberately leaves alternate faces, Rooms, prototype and face-down casts outside this filter. Those paths and full target/cost semantics remain unverified. The candidate exists only in the disposable local Git repository. No candidate runtime was built, no post-repair game was played, and no production files were installed or changed. The overall warning inventory includes other cancellation/fizzle events that have not been proved to share this cause.
