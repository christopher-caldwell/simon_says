# Synthetic opening-hand simulation fixture

This fixture shuffles a synthetic 60-card deck and counts lands in seven-card opening hands. It has no card rules, game actions, opponents, or connection to the real Magic simulator. It tests workflow reuse and is not evidence about real Magic simulation integration or deck quality.

Copy the directory into a new Git repository and commit it before running. It uses Python 3, Git and the `SIMON_PACKET` protocol. `experiment.json` normally sets `exercise` to `normal`; integration tests also use `review_once`, `block_once`, `reviewer_block_once` and `always_block` to test routing without external effects. The deterministic reviewer recomputes results in a separate action using the same fixture code, so it does not establish independent domain judgment.
