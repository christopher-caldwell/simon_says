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

## Recorded automatic dispatch

A live run from `../magic-dynamic-project-v2` to `../magic-dynamic-run-v2` completed automatically with three stage transitions. Its first Worker ran the historical source regression and committed `issues`; an independent Reviewer reproduced that issue and selected the declared `issues` route. The controller dispatched the actual configured `gpt-6.1-terra/high` Fixer in Codex thread `01a1989a-4a80-7a40-ad49-58d6d64d5c64`, which committed repair `eb75d6ba4ee386108e167aaba4312ef3054b4d0d`. A separate Reviewer passed all three frozen regression targets, and the automatic return to `observe` independently reran the real regression before the Reviewer selected `clean`. Final accepted commit `6ca9b13f7bc132e5f90598dc2846a92f29dc7f1c`; snapshot digest `e9c2cdb5a603b25b64dba56c22fb50b849beea0cd32a6cc056dfc164d7ee9d1e`. The Fixer reported 845,127 input tokens including 782,336 cached and 3,936 output; monetary cost was not recorded. This outcome demonstrates evidence-driven dispatch and a real source-level regression repair; it did not launch a new deck game or accept/install a native runtime.
