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

## Recorded live diagnostics

On 2026-11-01 four independent seed namespaces were run against isolated Manabrew runtime identity `d031727b7a21ea20ce38d9ea6b96f1629756813a8f7ad5e66573d4e1fd7e85dc`. The configured Codex roles were Investigator `gpt-6.1-terra/xhigh`, Supervisor `gpt-6-luna/high`, and fresh Analyzer `gpt-6.1-sol/high`. Each run assigned and retained four attempts, used one worker, and launched no replacement games. Native evidence and action records remain in `magic-full-run-{a,b,c,d}` beside the Simon Says checkout. The input counts below already include cached input; no monetary costs were recorded. Role-scope and scheduling limitations above apply to all four runs.

| Run | Outcome | Eligible / excluded | Eligible warning counts | Provider input / cached / output tokens |
| --- | --- | --- | --- | --- |
| A | Stopped at review limit after one correction attempt | 3 / 1 | 16 engine-warning, 16 card-play-failed | 995,210 / 876,288 / 8,054 |
| B | Awaiting approval; accepted `e6230dab40ca6851a750ba708f0b77fa72c42c36` | 3 / 1 | 5 engine-warning | 593,899 / 506,880 / 3,638 |
| C | Awaiting approval; accepted `97b007323fd2ae566d7b4ac22d4ba556bf26865c` | 3 / 1 | 1 engine-warning, 1 card-play-failed | 600,298 / 528,384 / 4,519 |
| D | Awaiting approval; accepted `cbd17c34a7ee785193054bfb22e58c5f4ed6e94a` | 4 / 0 | 4 engine-warning | 732,021 / 663,680 / 5,513 |

Warning categories and exclusion reasons can overlap; counts do not establish distinct affected games. A's Reviewer correctly refused two mismatching outputs: the Analyzer used retained attempts rather than eligible games for `verified_games` and added context to a then-exact issue array. The correction feedback was too vague, so the second candidate retained the mismatch and the controller safely stopped. B had explicit field guidance. C and D also allowed supplemental issue objects while preserving each source issue exactly; C exercised this successfully by adding a distinct finding. A's baseline predates those fixture improvements. B/C/D are three observed independent successful handoffs through the intentional approval gate, not proof of reliability for all workflows or a completed production study.

The induced no-game correction was run twice on preserved run B evidence. Both times the initial Analyzer deliberately omitted `excluded_count`, the Reviewer requested the missing auditable count, a new Analyzer added `1`, and exact-candidate review passed after one correction. They reached `awaiting_approval` without new games or evidence retirement. The first accepted commit is `9e4df5c0b16fc6314ca98f899e83e7a186947005`; its two provider turns used 811,879 input tokens (735,360 cached) and 6,415 output. The separately launched second accepted commit is `aff956224af9ef9bc70b2dc57b45f53be6ccb70f`; it used 637,946 input (546,816 cached) and 5,364 output. Its `DYLD_SHARED_REGION=avoid cargo build` ran at Git revision `34de8008d4c903deba29e8512d6ead75b189d439` with tracked files clean; the copied binary's SHA-256 was `85aa71a684360ec791a1673cffa5a39ce854db4cc7e7a38ac45f3407d6bbb932` both before and after execution. An unrelated untracked Fixer fixture source existed at build time. The first run's binary revision was not independently pinned. Original source evidence was protected by a 49-file SHA-256 manifest and verified before and after every native verification call.
