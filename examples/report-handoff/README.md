# Direct report handoff

A role runs a small command and reports its expected error. Simon follows the configured `issue` route and passes that report to the next role. This demonstrates a handoff; it does not actually repair software. Neither stage creates an output file or Git commit.

Copy these files into a clean Git repository and commit them, then run:

```sh
simon-says run workflow.toml --project . --run-dir /absolute/new/run
```

The configured roles use real model calls. Adjust their model settings for your installation. The completed role's report appears in the next role's packet as `last_transition.report`. A structured JSON-object report is passed as JSON text.
