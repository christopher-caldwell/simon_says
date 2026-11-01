# Explicit isolated Fixer repair fixture

This is a separate **user-requested** repair test; it is not an automatic next stage when a study reports an issue. It prepares a new Git repository, snapshots the actual Magic Fixer profile and model/effort, then asks that role to repair a deliberately defective, representative collector adapter. The adapter and issue are artificial; neither a real engine bug nor a real engine repair is asserted. No games, production engine, live backlog, runtime installation or research finalization are involved. The test overrides the source role's no-commit rule solely to allow Simon Says candidate checkpoints in the disposable repository.

```sh
python3 examples/magic-fixer/prepare.py /absolute/path/to/collection_plan /absolute/new/project
(cd /absolute/new/project && python3 -B -m unittest -v test_collector) # expected to fail before repair
simon-says run /absolute/new/project/workflow.toml --project /absolute/new/project --run-dir /absolute/new/run
simon-says status /absolute/new/run
```

The frozen reviewer runs independent acceptance controls against the exact candidate. The Fixer also supplies a new regression test and writes a bounded repair report. Passing proves the fixture repair and protocol handoff, not native engine acceptance or production repair.

## Recorded live run

On 2026-11-01, this fixture used source HEAD `756b01366d6844d6ba202e20afb5a3406abff690`, profile SHA-256 `39ac3579382c490c279bb73c1c308537353c9308dcdabf2eb33cd971a7466712`, and the profile's `gpt-6.1-terra` / `high` settings. Before the run, the three frozen controls produced the two expected failures. One `run` invocation created Codex thread `01a19856-3da0-70e2-8b23-789870b7527a`, which committed the bounded repair at `e971489dcdc15a1f798f741b56d2cbf92307e04c`; the controller's independent reviewer passed that exact candidate. A separate post-run invocation of `python3 -B -m unittest -v test_collector test_repair` passed all four tests in the run worktree. The controller reached `complete` without another role or a manual handoff. The Codex turn reported 89,309 input tokens (69,888 cached) and 947 output tokens; monetary cost was not recorded.

The retained local evidence is in `magic-fixer-project-v1` and `magic-fixer-run-v1` beside the Simon Says checkout. This result verifies the artificial fixture and role protocol only; it does not show a native simulator source fix, candidate runtime acceptance, or integration with a real recorded issue.
