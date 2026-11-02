# Model-selected repair loop

A real model runs `probe.py` and selects `issues` or `clean`. When the probe reports an issue, Simon dispatches the configured fixer and returns to the observer. Neither role is built into Simon; their profiles and routes are defined in TOML. There is no automatic Reviewer in this example.

Copy these files into a clean Git repository, commit them, then run `simon-says run workflow.toml --project . --run-dir /absolute/new/run`. This uses three real model calls in the normal repair case. Review `workflow.toml` to choose available model settings. The check and repair are deliberately small fixtures, not a real simulator.
