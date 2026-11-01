# Repeated bounded Magic role-handoff diagnostics

This fixture adapts the prepared ten-game fixture to **four native games per independent run**. It exercises real Investigator → Supervisor → fresh Analyzer invocations and an exact-candidate mechanical reviewer. It is a diagnostic of the handoff, not the Magic project's 100-game-per-arm comparative research process: it omits scheduled check-ins, makes no deck recommendation, and stops at an explicit approval gate without publishing or retiring raw data.

Unlike the strict ten-game diagnostic, this fixture permits verified exclusions. Collection still requires exactly four assigned and retained attempts, no retries, and native verification; it records actual eligibility, exclusions, and warnings. The reviewer independently invokes the native verifier and compares the collection's count and issue inventory to the original native analysis. The Analyzer must report limitations and preserve every issue. This only checks mechanical consistency; it does not certify rules or the Analyzer's domain judgment. Failed or interrupted actions must not be replayed without explicit recovery; external effects are not undone.

Prepare and run each diagnostic separately with a unique seed. Start from a clean prepared `examples/magic-roles-ten` project and its matching isolated runtime. Runs are intentionally sequential to avoid conflicting host process observations.

```sh
python3 -B examples/magic-full-runs/test_contracts.py -v
python3 -B examples/magic-full-runs/prepare.py /absolute/ten-game-project /absolute/new-project --seed unique-seed
SIMON_MAGIC_RUNTIME_ROOT=/absolute/isolated/runtime \
  simon-says run /absolute/new-project/workflow.toml --project /absolute/new-project \
  --run-dir /absolute/new-run --context /absolute/new-project/original-task.txt
simon-says status /absolute/new-run
```

The preparation clones the clean isolated input and commits frozen configuration, instructions, role profiles, schemas, native helper and verifier. Its game count and run identity are fixed before execution. The included tests use fake verifier results and temporary files; they launch neither providers nor games.

A separate controlled correction exercise reuses an existing run's retained evidence without launching games. Its initial Analyzer is intentionally instructed to omit `excluded_count`; the mechanical Reviewer requires that valid audit field, provides exact feedback, and the controller should route one correction. This induced omission is a test condition, not a claim of a model or domain defect. Original issues may be supplemented but not removed or altered. Evidence hashes and the exact accepted source checkpoint are bound before starting, and the new draft also stops at approval.

```sh
python3 -B examples/magic-full-runs/correction/prepare.py /absolute/source-project /absolute/run-awaiting-approval /absolute/new-project
SIMON_MAGIC_RUNTIME_ROOT=/absolute/isolated/runtime \
  simon-says run /absolute/new-project/workflow.toml --project /absolute/new-project \
  --run-dir /absolute/correction-run --context /absolute/new-project/original-task.txt
```
