# Native Magic game → configured Fixer → rebuilt runtime → fresh game

This isolated diagnostic exercises dynamic roles and routes on **real newly launched native games**, the actual configured Magic Fixer, a candidate runtime built from the exact reviewed repair commit, and a second native game batch. It intentionally repeats the previously observed ten-game seed/configuration in **new action-owned stores** for a controlled before/after comparison; this is a separately authorized diagnostic, not independent sampling or a replay of an uncertain action. It never modifies the Magic or Manabrew source checkouts, installs a runtime, publishes findings, or retires evidence.

The preparer verifies the historical run's runtime ID, config, all four deck/GUIDE pairs, and the frozen native package/receipt. Baseline source and original game evidence are read-only. A new ten-game batch must independently reproduce the exact scoped Count expression failure to select `issues`; other exclusions/unknown errors, missing target issue or incomplete evidence cause a durable stop rather than a fabricated repair. All diagnostic counts and warning categories remain in `game.json`. The model Fixer works in a disposable engine worktree; Reviewer requires three frozen regression targets, including independently discovered prevented-move and bottom-of-library cases. The build Worker then uses the maintained native setup against the exact reviewed repair commit, writing its runtime, gate logs and receipt only to its action scratch. Independent review verifies source binding, immutable binary/archive hashes and native doctor before the controller runs the second batch. If the scoped issue remains, the frozen route permits another bounded repair; other errors or exclusions stop for inspection. Passing does not certify all Magic rules or prove previous game's outcomes.

```sh
python3 -B examples/magic-dynamic-game/test_contracts.py -v
python3 -B examples/magic-dynamic-game/prepare.py /absolute/manabrew /absolute/collection_plan /absolute/retained/run /absolute/isolated/baseline-runtime /absolute/prepared-ten-game-project /absolute/new/project
simon-says validate /absolute/new/project/workflow.toml
simon-says run /absolute/new/project/workflow.toml --project /absolute/new/project --run-dir /absolute/new/run
simon-says status /absolute/new/run
```

The action sandbox owns runtime stores and game evidence. Git reset cannot undo external effects: after interruption, do not replay an uncertain build or game without explicit recovery. Native setup can require 10–20 minutes and several gigabytes per build; online provider authentication and offline-cached Cargo dependencies are required. This is a small experimental workflow, not a production comparative study. Some same-user IPC and native process behaviors remain outside the filesystem guard.
