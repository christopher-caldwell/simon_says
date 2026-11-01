# Structured expense audit with dynamic routing

The same Simon Says dispatcher audits a tracked expense dataset. If valid amounts contain surrounding whitespace, its reviewer selects the declared normalization route, an authored normalizer role corrects the records, and the workflow re-audits. A clean dataset skips normalization. Invalid values stop instead of being guessed. Currency amounts are checked and summed as integer cents. This example runs no models or Magic software.

Copy this directory into a new Git project, commit it, then run `simon-says run /path/to/workflow.toml --project /path/to/project --run-dir /path/to/new-run`. The controller keeps the source checkout untouched.
