# Worker-selected dispatch fixture

This small, generic example shows a workflow role choosing which declared role
runs next. The observation Worker runs a command that initially exits with an error, writes an observation, and selects `issues`
or `clean`. Its Reviewer checks the exact candidate and proposed route. If the
Worker reported an issue, Simon dispatches the configured repair role, reviews
the repair, then returns to observation. A transition limit bounds the loop.

To run it, copy these files to a clean Git repository, commit them, then run
`simon-says run workflow.toml --project . --run-dir /absolute/new/run`. The
initial data exercises repair; set `engine.json` to `{"correct":true}` before
the initial commit to skip it. Set `mode.json` to `{"mode":"permanent"}` to
exercise the transition limit. See the run's action packets and `state.json`
for each role, route, and checkpoint.

These are deterministic fixture roles, not model calls or a Magic simulator.
To use models, replace role definitions with agent profiles and provide
instructions to produce the same declared artifacts and routes.
