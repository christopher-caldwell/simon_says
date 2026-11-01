# Deterministic data-report fixture

This two-stage example validates integer measurements and creates an independently reviewed summary. Copy the entire directory into a fresh Git repository and commit it as the baseline. The command role reads an action packet from the absolute `SIMON_PACKET` environment path and writes a single result envelope to stdout. A worker creates and commits its stage's output; a reviewer recomputes the result from the input and reports its exact Git HEAD. The Unblocker stops on invalid input. The script uses only Python 3 and Git.

The fixture is not a substitute for a live model-provider test. Running the two stages through Simon Says remains subject to controller integration and Git isolation being implemented and verified. Input and outputs reside in the fixture repository, while action packets must be stored outside the worktree.
