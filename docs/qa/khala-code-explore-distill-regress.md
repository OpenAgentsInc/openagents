# Khala Code Explore -> Distill -> Regress

Status: implemented for the fixture regression tier in
`packages/khala-qa-harness`.

Issue: [#8039](https://github.com/OpenAgentsInc/openagents/issues/8039)

## Contract

Explore sessions from the desktop drivers are represented as
`khala_code_qa_explore_session.v1`: ordered driver actions, the backend tier,
the driver mode, source run status, and the oracle expectations observed during
the exploratory run.

`distillKhalaCodeQaExploreSessionToRegression` is the promotion boundary:

- a session with at least one replayable non-boot action and at least one oracle
  expectation emits a `khala_code_qa_distilled_regression.v1` scenario;
- the emitted scenario is deterministic data and replays through the normal
  `runKhalaCodeQaScenario` gate;
- a session without replayable actions or without oracle evidence returns
  `INCONCLUSIVE` and does not include a scenario.

INCONCLUSIVE is deliberate. A discovery that cannot be reduced to replayable
actions plus oracle evidence is not shipped as a regression and must not be
counted as confirmed QA coverage.

## First Committed Regression

The first distilled regression is committed into the seed corpus as
`scenario.khala_code.distilled.q6_2_first_fleet_panel_distilled_regression.v1`.
It was distilled from a fixture desktop explore session that opened the fleet
panel, read `codexFleetStatus`, and asserted the fleet-count projection remains
crash-free and schema-valid.

It lives in the `distilled_regressions` seed-corpus group and therefore runs
with:

```sh
bun run --cwd packages/khala-qa-harness test
```
