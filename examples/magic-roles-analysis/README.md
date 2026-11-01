# Analysis-only continuation of a stopped diagnostic

This narrowly scoped example recovers analytical value from the recorded ten-game case with 10 retained attempts, 6 eligible games, and 4 exclusions for unsupported engine behavior. It starts a new workflow; it does not resume, alter, or retry the original stopped run. The source profile, original task, candidate design, native evidence, exclusions, and eligible warnings are preserved. The Analyzer and deterministic Reviewer independently invoke the native verifier on a disposable SQLite mirror that points at the original protected attempt bytes. The original store's full file inventory and hashes are checked before and after verification. The draft requires explicit approval and does not publish or retire evidence.

This example intentionally refuses different counts or exclusion reasons; author a new workflow for a different result rather than treating this specific analysis as generic. The native integrity verifier does not certify game rules, endpoint coverage, or the Analyzer's judgment. Verification may create files only in action-owned scratch. A failed preparation can leave an incomplete new project; inspect or remove that new project before retrying. Never run collection or resume games as part of this workflow.

```sh
python3 -B examples/magic-roles-analysis/prepare.py /absolute/ten-game-project /absolute/stopped-run /absolute/new-analysis-project
SIMON_ANALYSIS_PROJECT=/absolute/new-analysis-project python3 -B examples/magic-roles-analysis/test_contracts.py -v
SIMON_MAGIC_RUNTIME_ROOT=/absolute/isolated/runtime \
  simon-says run /absolute/new-analysis-project/workflow.toml --project /absolute/new-analysis-project \
  --run-dir /absolute/new-analysis-run --context /absolute/new-analysis-project/original-task.txt
simon-says status /absolute/new-analysis-run
```

Preparation checks the source project's clean Git commit, stopped run identity, original question and native denominators, and records their provenance. It refuses an existing destination and hashes the complete evidence tree twice to detect concurrent changes. The tests do not run a provider or simulation; separate native verification does not launch games.
