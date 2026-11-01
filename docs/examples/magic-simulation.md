# Example: Magic deck simulation workflow

**Status:** illustrative validation target; no claim that these file names or scripts match the current Magic repository. The implementation team must inspect and adapt to actual interfaces before executing it.

## Scenario

A user creates or updates a Magic: The Gathering Commander deck. Existing simulation tooling should evaluate it, check the results, correct routine problems, and produce an analysis. A high-intelligence agent should not be doing repetitive command orchestration while scripts or smaller models can do the work.

## Proposed stage contracts

### Stage `simulate`

**Starting inputs:** a decklist, simulation settings, and installed simulation tooling.

**Worker:** deterministic command script or low-cost agent that calls the existing simulator, records the exact command/settings, and writes structured results.

**Required output:** simulation results with deck identity, settings, run count, and the metrics the real simulator actually supports. Avoid fabricated statistics or schema fields. Output shape should be agreed from real simulator output.

**Mechanical checks:** output exists, is Git-visible at the reported candidate commit, parses under its frozen schema, reports the expected deck identity, and meets any declared executable verifier checks (e.g. observed trial count).

**Reviewer:** checks whether the experiment was run correctly, whether the measurements answer the user's question, whether bias or missing data requires changes, and whether conclusions are supported. `pass` routes to `analyze`; `changes_required` returns concrete correction guidance; `blocked` invokes Unblocker.

**Unblocker:** diagnoses missing tooling, incompatible input, environmental failures, or an unresolved assumption. It does not invent replacement simulation results. If a correction can safely enable retry, it returns `retry`; otherwise stops with evidence.

### Stage `analyze`

**Inputs:** the accepted simulation output at the exact earlier checkpoint.

**Worker:** analytic agent (potentially higher effort) with an explicit profile for interpreting results, limitations, and comparison goals.

**Required output:** an analysis artifact that references the exact simulation results and distinguishes observation from recommendation. Optionally a machine-readable summary.

**Mechanical checks:** required output exists, references recorded source identity, parses under the chosen schema, and is part of a candidate commit.

**Reviewer:** determines whether the interpretation accurately reflects the experiments and supports claims. Pass completes the workflow; changes required returns to the analyst.

## Example routing

```text
simulate.Worker
  -> mechanical output validation
  -> simulate.Reviewer
      pass -> analyze.Worker
      changes_required -> simulate.Worker
      blocked -> simulate.Unblocker -> retry simulate.Reviewer / stop

analyze.Worker
  -> mechanical output validation
  -> analyze.Reviewer
      pass -> COMPLETE
      changes_required -> analyze.Worker
      blocked -> analyze.Unblocker -> retry / stop
```

For a Worker blocker, retry begins at the original Worker gate. For a Reviewer blocker, retry begins at Review and retains the exact validated candidate unless the explicit correction changes it. The controlling commit and feedback must be unambiguous.

## Sample model-authored template sketch

The [template specification](../design/workflow-template.md) contains an illustrative TOML file. The example's actual simulator, paths, and contracts are intentionally left to a model *after reading the real Magic repo*. That is central to this product: the runtime doesn't require one fixed deck simulation output schema or a hardcoded Magic phase.

## Acceptance exercise

1. Run a valid deck from a known clean Git baseline.
2. Deliberately cause Reviewer to reject a materially incomplete simulation result; confirm full feedback returns to the Worker, not just a bare PASS/FAIL token.
3. Induce a legitimate recoverable block, such as a missing dependency in a controlled fixture. Confirm the Unblocker gets originating gate and context and attempts one bounded retry.
4. Induce a second block; verify a clean stop, not recursive AI orchestration.
5. Kill the controller after persisting `running` but before the action completes; fresh `status` and `resume` must not guess that it is safe to replay.
6. Confirm every accepted stage has a precise Git commit and that no unreviewed candidate is presented as final output.

## Domain-specific questions to settle from real code

- What are the actual simulator entry points, CLI flags, inputs, and output locations?
- Are outputs deterministic/reproducible, and what random seed belongs in the contract?
- Which metrics are emitted versus inferred in analysis?
- Do simulations produce huge intermediate artifacts that should remain outside Git?
- Can all v0 tool operations stay within the Git worktree, or are external services involved?

The model-authoring guide should collect these answers from the repository and form the concrete workflow. Do not hardcode guesses into Rust.
