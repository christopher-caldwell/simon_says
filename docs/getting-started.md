# Getting started with Simon Says

Simon runs a workflow that you define. You describe the roles, their instructions, what they receive and produce, and where each stage may go next. Simon launches the configured roles, checks the declared files and checkpoints, and follows the workflow. The models or commands do the actual work and make domain judgments.

## Start with an example

- [CSV questions and analysis](../examples/csv-questions/README.md): three models prepare questions, run code, and answer from the results.
- [Magic simulation](../examples/magic-study/README.md): four roles prepare a simulation, run it, optionally fix reported engine issues, and analyze the results.
- [Deterministic data report](../examples/data-report/README.md): a local command example that does not require a model.

The examples show workflow definitions, role instructions, and expected files. Model examples make real provider calls. The Magic template also needs the project's simulator and deck data.

## The pieces of a workflow

A workflow has a name, a starting stage, named roles, and stages. A role specifies how to launch a model or command. A stage gives that role instructions and declares the files it needs and produces. `on_pass` names the next stage. Named `routes` let a role select from destinations the workflow already permits.

For example, the Magic Supervisor can return `fix` or `analyze`. Simon does not decide whether a Magic engine issue is real. It reads the model's structured choice and follows the matching declared route.

The workflow's TOML file and referenced profiles, instructions, and schemas are frozen at the start. Template references are relative to the workflow file; stage input and output paths are relative to the Git project root. Add files to `frozen_project_files` when their tracked contents must remain fixed for the run. Declare `protected_inputs` when a later stage must not change an earlier stage's files.

Every role receives an action packet that includes the role profile, stage instructions, declared files, the prior handoff, and optional original task context. Define the job in the profile: do not assume Simon knows what an Investigator, Supervisor, Fixer, or Analyzer should do.

## Run and inspect

Install Rust and install the CLI from this repository with `cargo install --path .`. For model roles, install and authenticate the configured provider CLI. Prepare a clean Git project containing the files the workflow needs, then run:

```sh
simon-says validate /path/to/workflow.toml
simon-says run /path/to/workflow.toml --project /path/to/project --run-dir /path/to/new-run --context /path/to/question.txt
simon-says status /path/to/new-run
```

The run directory must be new and outside the project checkout. The context file is optional. Simon creates an isolated worktree inside the run directory and keeps the action packets, model responses, logs, and state there. One `run` command continues until the workflow finishes, reaches a configured stop, or encounters an error.

When a stage has routes, its model must return the selected label in the top level `route` field of its response. Simon permits only the declared labels. Reviewers and Unblockers are optional; define them only when the workflow needs them.

## When a run stops

Use `status` to see the current stage and reason. A blocked role without an Unblocker stops the run; the workflow does not silently skip it. A failed or interrupted action is not automatically replayed. Read the action records before deciding how to recover. The [recovery guide](design/recovery.md) covers reset and adoption commands.

The controller checks execution mechanics, including the files and checkpoints the workflow declares. A successful run does not prove that a model's judgment or an external tool's results are correct. Add review instructions or deterministic checks when your task needs them.
