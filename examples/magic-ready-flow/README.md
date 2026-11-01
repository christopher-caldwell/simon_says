# Real four-role diagnostic, repair, rebuild, and warning triage

This bounded isolated diagnostic exercises actual configured Investigator → Supervisor → Analyzer → Fixer profiles in one controller invocation, then independently reviews the repair, builds the exact accepted source commit with native setup, and launches a new native verification batch. Previously reviewed design, game, analysis and repair artifacts are protected during subsequent actions. The same configured seed is deliberately repeated in distinct action-owned stores within a run as a before/after diagnostic, not independent statistical sampling. A separate diagnostic may supply an optional `--seed` during preparation; its actual outcomes must be recorded without retrying a non-reproducing case. No production runtime is installed, no research is published, and no raw evidence is retired.

The post-repair reviewer routes a scoped-target recurrence or any remaining warning inventory to a fresh Analyzer triage, which independently verifies the evidence and records its full warning inventory and a concrete next action without assuming warnings are harmless. That draft is mechanically reviewed and accepted, then an escalation action stops durably: no automatic arbitrary repair is attempted without a verified scope and acceptance criteria. The workflow can complete only when the scoped issue is absent and the retained warning inventory is empty. **Neither a completed diagnostic nor the scoped regression tests establish general Magic rules correctness or production readiness.** Unknown errors, inconsistent evidence, uncertain actions, and non-reproducible baseline conditions stop safely. Git cannot undo external effects.

Preparation requires macOS, authenticated native Codex, the historical native runtime and clean prepared game project described in `../magic-dynamic-game/README.md`, and an absent target path. The current user's role files are read and snapshotted; their configured model/effort values are retained. A candidate rebuild requires significant time and disk space.

```sh
python3 -B examples/magic-ready-flow/test_contracts.py -v
python3 -B examples/magic-ready-flow/prepare.py /absolute/prepared-game-project /absolute/collection-plan /absolute/new/project
simon-says validate /absolute/new/project/workflow.toml
simon-says run /absolute/new/project/workflow.toml --project /absolute/new/project --run-dir /absolute/new/run --context /absolute/new/project/original-task.txt
simon-says status /absolute/new/run
```

A failing or interrupted native game/build may have had external effects; preserve its scratch evidence and do not replay without explicit recovery. The constrained roles and lack of scheduled Desktop check-ins mean this is not the complete Magic production study process. Native role judgements and card interactions are not certified by mechanical schemas.
