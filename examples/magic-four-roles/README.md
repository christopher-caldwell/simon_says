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

## Recorded real run

On 2026-11-01 a single uninterrupted controller command completed at `../magic-four-roles-run-v1` with no manual stage advancement, yielding accepted commit `3b0f72cbeef857612a8deef8da2bfeb044ee2319`. It used prepared baseline `14c10036d06d5e27d13af53dc7bd42b36b021652` and context-bound workflow digest `f8c4b54f9214f0783246a0a03616cec5db53c5bd2446cd0f77ca662e96fe7f83`. The locally copied executable `../simon-four-pinned` was built from controller revision `4e781ba6e154fc979bd4519a483e5a9022597e59`; its SHA-256 was `71e45e043d275bface3e6b3520ce2f6dce8ddce411e7ca4260215776f78c8fc4` both before and after the run. The preparation captured role/guidance source HEAD `756b01366d6844d6ba202e20afb5a3406abff690`; captured source files may include the user's pre-existing uncommitted changes, and the frozen profiles contain exact source hashes.

| Order | Role and settings | Native thread | Accepted Worker candidate | Input / cached input / output tokens |
| --- | --- | --- | --- | --- |
| 1 | Investigator `gpt-6.1-terra/xhigh` | `01a198d7-a460-7253-8a5d-f2ec0023fe0c` | `4799611fb640a41a25edb38d8ba53d99120baa8f` | 225,315 / 194,304 / 1,273 |
| 2 | Supervisor `gpt-6-luna/high` | `01a198d8-4433-7290-98f5-87d0202dbd1c` | `74f02b4ca217c244c450ce1542939a2f621625b9` | 129,875 / 102,144 / 486 |
| 3 | Analyzer `gpt-6.1-sol/high` | `01a198da-50e4-7080-a988-1459f3278fd3` | `9d213edf2e3dc137328459d0be0e9f97ce24dabe` | 405,557 / 369,920 / 3,278 |
| 4 | Fixer `gpt-6.1-terra/high` | `01a198db-e635-7d30-bd63-17dabefeb094` | `3b0f72cbeef857612a8deef8da2bfeb044ee2319` | 1,176,467 / 1,124,096 / 4,798 |

Each candidate was independently reviewed; packet and response records are under `actions/`. The Supervisor ran ten new games against the isolated historical runtime: ten retained attempts, six eligible and four excluded for the scoped unsupported Count expression, individually attributed to retained stderr. All-attempt warning flags were six `engine_warning` and four `card_play_failed`; eligible-only flags were three each, and categories can overlap. The fresh Analyzer independently re-verified the evidence, answered the frozen question with those limitations, and identified the scoped repair. Its exact-candidate mechanical Reviewer selected the declared `issues` route, causing the controller to dispatch Fixer automatically. No approval was issued or needed to run the expressly requested isolated repair; the draft was not finalized or published and no evidence was retired.

The Fixer committed a source-level repair and reported a passing actual archived-card expression check; the separate review action independently passed all six tests across the three frozen `left_battlefield_this_turn`, `bottom_departure`, and `prevented_zone_move` targets. The reviewed `design.json`, `game.json`, and `analysis.json` retained identical Git modes and blob IDs through the final commit: respectively `f93815f2fa357e10c260435eeca64928b5b0a000`, `b18b381db32cc5c54ada99d43f40318e5ed0eb1b`, and `df7c9363effec6438c95afca77b0c1b61fab224d`. They were declared protected inputs at the later handoffs. The recorded model input totals include cached tokens; monetary cost was not recorded.

This run did **not** build or install a candidate runtime or run games after repair. The single repeated seed is not independent statistical replication. The repair supports the observed selector only, not all possible selectors; broader engine correctness and the remaining warnings are not certified. The controller's successful role handoff is not proof that these four Codex exec threads appear as separate Desktop chats or that the scheduled production research process was performed.
