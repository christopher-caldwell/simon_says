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
candidate. The command Reviewer checks the candidate identity only: it does
not verify whether the code actually ran or whether the answers are correct.
Inspect the model action logs for executed commands and use an appropriate
workflow-defined reviewer if independent domain verification is required.
Model names are local provider configuration and may need to be changed for a
different installation. This example uses real model calls and consumes tokens.
