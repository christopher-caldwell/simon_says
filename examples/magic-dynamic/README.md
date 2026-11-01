# Dynamic routing over a real Magic engine reproduction

This diagnostic connects **a verified issue → the configured real Fixer → independent repair review → a fresh regression** in one controller run. It is rooted in actual retained Magic game diagnostics and runs the actual historical Manabrew engine source, but the observation is an executable regression, **not a newly simulated deck game**. It does not install a runtime, alter the original engine or Magic checkout, or claim the historical game's outcome would change. Run it only in a new isolated project with an explicitly prepared Simon Says dynamic-routing build.

The preparer reads the historical engine commit from Git and four original retained game stderr files without modifying either source, records their hashes, freezes the exact regression and current Fixer profile, then creates a disposable Git project. The `observe` Worker runs the frozen regression against the checked-out historical source. Its independent Reviewer reruns it: only the exact expected unrecognized-expression failure routes `issues` to the configured Fixer, while both tests passing routes `clean` to completion. Compilation errors, partial failures and unknown outcomes stop without guessing. The Fixer may change only the isolated engine and repair report; its Reviewer independently executes all three frozen regression targets, including previously discovered prevented-move and bottom-of-library cases. Its successful pass routes back to `observe`; the workflow-wide transition limit bounds cycles. External actions and model invocations must never be silently replayed after interruption.

```sh
python3 -B examples/magic-dynamic/test_contracts.py -v
python3 -B examples/magic-dynamic/prepare.py /absolute/manabrew /absolute/collection_plan /absolute/retained/run /absolute/new/project
simon-says validate /absolute/new/project/workflow.toml
simon-says run /absolute/new/project/workflow.toml --project /absolute/new/project --run-dir /absolute/new/run
simon-says status /absolute/new/run
```

The regression uses offline cached Cargo dependencies and compiles separately in action-owned scratch. Each action can require minutes and substantial disk space. Native model authentication for the configured source Fixer profile is required. A successful run validates the specified engine regression and dynamic dispatch; it does not prove actual simulation-to-Fixer-to-new-simulation, full Magic rules, candidate-runtime acceptance, or a production study.
