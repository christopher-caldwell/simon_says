# CSV questions, code, and analysis

This workflow defines three model roles in TOML. The first reads `data.csv` and
writes questions. The second executes code to gather evidence and records the
snippets and observed results. The third answers the questions using that
evidence. Simon dispatches the roles and passes their tracked outputs through
the three stages in one run.

Copy these files into a clean Git repository, set the model names and effort in
`workflow.toml` as needed, replace `data.csv` if desired, and commit the files.
Then run:

```sh
simon-says validate workflow.toml
simon-says run workflow.toml --project . --run-dir /absolute/new/run
```

Results appear in the run worktree as `questions.json`, `evidence.json`, and
`answers.json`. The controller checks the JSON shapes and each exact Git
candidate. This example has no Reviewer: inspect the model action logs for executed commands. Add a workflow-defined Reviewer if independent verification is required.
Model names are local provider configuration and may need to be changed for a
different installation. This example uses real model calls and consumes tokens.
