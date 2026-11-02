# Magic simulation example

This example shows the four roles from a Magic study:

1. The Investigator prepares the deck inputs, pod, game counts, and runnable configurations.
2. The Supervisor runs those configurations, watches the simulations, and records issues.
3. The Fixer repairs issues from the run when the Supervisor selects `fix`.
4. The Analyzer answers the original question after the Supervisor, or after the Fixer.

The profiles say what each model should do. The TOML file says which role runs at each stage, which files pass between stages, and which routes Simon may follow. Simon does not need Magic rules in its code.

## Try it with your Magic project

This is a template, not a bundled simulator. It needs a clean Git checkout containing the Magic decks, the simulation tools, and any runtime setup described by that project. It uses real model calls. The provided question requests at most six games, but executing them consumes model tokens and local computing resources. Edit the question and profiles to match the actual project before running.

Copy this directory into your project's `simon-example` directory and copy `question.md` to `study/question.md` at the project root. Commit both directories to the project. Change model names in `workflow.toml` if needed. From the project root:

```sh
simon-says validate simon-example/workflow.toml
simon-says run simon-example/workflow.toml --project . --run-dir /absolute/path/to/a/new-run --context study/question.md
simon-says status /absolute/path/to/a/new-run
```

The template and profile paths are relative to `workflow.toml`. The `study/...` inputs and outputs are relative to the project root. The model must create the three configuration files in the Investigator stage; Simon checks they exist before dispatching the Supervisor. The example does not dictate the simulator's config schema, because that belongs to the Magic project.

After a successful run, inspect `study/design.json`, `study/supervision.json`, and `study/analysis.json` in the run's `worktree`. `study/repair.json` exists only if the Fixer was dispatched and completed. A blocked Fixer stops this workflow before analysis. To analyze partial results after a blocked repair, prepare an explicit continuation with the preserved evidence; the original run does not automatically skip the blocked role.

The [Magic walkthrough](../../docs/examples/magic-simulation.md) describes the real local studies and their limits. The older [four role fixture](../magic-four-roles/README.md) uses a different review sequence and has its own requirements.
