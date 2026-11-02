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

## What the roles do

The question writer reads the CSV and writes `questions.json`. The code runner uses those questions, executes code, and writes `evidence.json`. The analyst reads both files and writes `answers.json`. The three role profiles are ordinary text files; change them when your task needs different behavior.

The code runner owns the code it executes. In one local test, a deliberately broken calculation failed, the model inspected and fixed it in the same action, and then produced evidence for the analyst. Simon did not diagnose the calculation or dispatch a separate Fixer. That is one observed run, not a guarantee that a model will resolve every error. See the [test evidence](../../docs/implementation/evidence.md).
