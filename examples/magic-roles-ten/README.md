# Ten-game role diagnostic

This reproducible adaptation of `examples/magic-roles` exercises the same actual Investigator, Supervisor, and fresh Analyzer with ten assigned native diagnostic games, one worker, no retries and a fresh seed namespace. It preserves the same isolated runtime and explicit reviewed-draft approval gate. It does not claim to be the 100-game-per-arm measurement study described by the Magic project, or validate domain and rule correctness.

First follow the sibling example's instructions to prepare an isolated runtime and a clean, committed one-game role project. Then:

```sh
python3 -B examples/magic-roles-ten/prepare.py /absolute/one-game-project /absolute/new-ten-game-project
SIMON_MAGIC_RUNTIME_ROOT=/absolute/isolated/runtime \
  simon-says run /absolute/new-ten-game-project/workflow.toml --project /absolute/new-ten-game-project \
  --run-dir /absolute/new-run --context /absolute/new-ten-game-project/original-task.txt
simon-says status /absolute/new-run
```

The patch requires the sibling fixture's exact expected content and fails on drift; re-check any rejected hunk rather than forcing it. `prepare.py` creates a separate local clone and commit. Its checks bind schema counts and the config, and do not launch games. As with the sibling example, do not point at a production store, do not automatically replay interrupted actions, and do not approve finalization on the user's behalf. The diagnostic rejects any exclusion, retry or shortfall and retains the scratch evidence for investigation.

After preparation, run the focused contract tests without invoking a provider or starting a game:

```sh
SIMON_TEN_PROJECT=/absolute/new-ten-game-project python3 -B examples/magic-roles-ten/test_contracts.py -v
```
