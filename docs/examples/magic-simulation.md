# Example: a Magic simulation

A Magic study can use the same Simon controller as a CSV study. The workflow describes the roles and their handoffs; the models use the Magic project's tools and rules. Start with the [Magic example](../../examples/magic-study/README.md).

The intended flow is:

```text
Investigator prepares the payload
              ↓
Supervisor runs the simulation and records issues
              ↓
       Is repair needed?
       yes ↓       no ↓
         Fixer     Analyzer
           ↓
         Analyzer
```

The Investigator prepares the actual deck inputs, opponent lists, pod, game counts, and runnable simulator configurations. The Supervisor runs that payload, monitors the games, and records engine issues from this run. The Fixer works on the reported engine issues when the Supervisor selects `fix`. The Analyzer interprets the results and answers the original question.

Simon provides the role's profile, stage instructions, declared inputs, and any run context to each dispatched model. The profile must describe that role's actual responsibility. A profile file named in a workflow is read and frozen by Simon; other project instructions are not automatically substituted for that profile. If a role needs another file, say so explicitly and ensure it is available in the project checkout.

The example assumes a real Magic project with its simulator and decks already available. `simon-says validate` checks the template; it does not prove the simulator is installed, the models will follow instructions, or the games are correct.

## What happened in the local studies

A six game test used the same four role design, with two games each for Giada CP3, Miirym CP3, and Miirym CP4. Simon dispatched Investigator, Supervisor, and Analyzer in one run. The Investigator produced the three configurations; the Supervisor ran all six games and reported warnings. It did not identify a confirmed issue that required repair, so the Fixer route was not exercised. The run completed at `magic-role-boundary-run-20261102`, accepted commit `5a86625bc9c696b73767a0883e0361d0fd830020`.

A later study assigned 300 games to each checkpoint. It recorded 299 eligible Giada games, 300 eligible Miirym CP3 games, and 300 eligible Miirym CP4 games. One Giada game timed out and was excluded. The Supervisor routed that issue to the Fixer; the Fixer could not establish a cause or a verified repair and returned blocked. Simon stopped as configured. The Analyzer was started in a separate Simon continuation with the preserved results and the Fixer's report. No games were repeated for that continuation. The original run is `magic-900-study-exact-20261102`; the analysis continuation is `magic-900-analysis-20261102`.

The larger study demonstrates automatic dispatch to the Fixer, but not an uninterrupted complete workflow through analysis after a blocked repair. The game counts do not certify the simulator's rules or prove that the decks are optimized. The analysis compared selected lists in a particular opponent pod, not alternative versions of those lists.
