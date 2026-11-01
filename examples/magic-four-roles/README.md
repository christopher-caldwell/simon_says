# Four real Magic roles in one diagnostic

This isolated workflow runs the actual configured Investigator → Supervisor → fresh Analyzer → Fixer profiles in one Simon Says command. The original user explicitly requested this bounded four-role diagnostic, including a separately scoped Fixer for the reviewed issue. The Analyzer produces an unfinalized draft, and the Fixer may work on the reviewed engine issue without publication approval. This test never publishes, retires evidence, installs a runtime, changes the user's source checkouts, schedules checks or creates verified Desktop chats.

The Supervisor launches one fresh ten-game native diagnostic in action-owned storage, using the previously observed seed in a distinct store. It retains all attempts and warnings, and the deterministic Reviewer independently verifies native evidence. Analyzer independently re-verifies and answers the frozen original question before a Reviewer routes the exact reviewed issue to the configured Fixer. The Fixer's protected inputs include the actual reviewed game and analysis; the independent repair Reviewer runs three frozen real-engine regression targets. If no scoped issue occurs, the declared route ends without fabricating a repair. Unknown errors and uncertain actions stop. This is an isolated diagnostic, not the full 100-per-arm research process, scientific replication, comprehensive rules certification, or proof that every reported warning was repaired. A source-level repair is not a rebuilt/installed runtime.

Prepare an already isolated, clean game project from `examples/magic-dynamic-game`, then run:

```sh
python3 -B examples/magic-four-roles/test_contracts.py -v
python3 -B examples/magic-four-roles/prepare.py /absolute/prepared-game-project /absolute/collection_plan /absolute/new-project
simon-says validate /absolute/new-project/workflow.toml
simon-says run /absolute/new-project/workflow.toml --project /absolute/new-project --run-dir /absolute/new-run --context /absolute/new-project/original-task.txt
simon-says status /absolute/new-run
```

Preparation captures the current source role TOMLs, model settings and applicable guidance through the existing provenance-aware role preparer, adapts only bounded diagnostic dispatch/scheduling/finalization and explicitly authorized Fixer scope, and freezes the resulting profiles. The game config intentionally uses the recorded seed, with a separate store, and must not be silently retried. Runtime setup and source dependencies must already be locally prepared and the selected models authenticated. Git cannot undo external effects; inspect interrupted actions before explicit recovery. Per-action logs, candidate commits and native evidence remain in the run directory.
