# Evaluation And Regression System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #52 from the Bun/Effect terminal-agent systems list. It defines
how terminal-agent quality should be measured across tasks, models, adapters,
tools, prompts, costs, and safety regressions.

## Target

Build an evaluation system that can replay task suites, compare model/provider
adapters, record first divergence, protect against regressions, and report
results without leaking private task data.

## User-Visible Capability

Users and maintainers should be able to:

- Run a regression suite before release.
- Compare providers and adapters on the same task shape.
- See failure classes, cost, duration, and evidence refs.
- Promote a fixture from a real failure after redaction.
- Know whether a model upgrade changed behavior.
- Keep private customer and repository work out of public eval reports.

## Evaluation Model

Each eval run should include:

- Eval suite ref.
- Task fixture refs.
- Adapter and provider refs.
- Runtime version.
- Tool policy.
- Budget policy.
- Result verdict.
- First-divergence ref.
- Artifact refs.
- Cost and latency summary.
- Safety and redaction verdicts.

Results should separate "solved task", "followed policy", "used budget
correctly", and "produced public-safe evidence".

## Bun/Effect Boundary

Use Effect services for:

- `EvalSuiteService`: loads and validates task suites.
- `EvalRunnerService`: executes fixtures under controlled layers.
- `EvalComparatorService`: computes deltas and first divergence.
- `RegressionGateService`: enforces release thresholds.
- `EvalReportService`: emits public-safe and private reports.

Use Schema for fixtures and reports. Use deterministic clocks and memory
stores where possible. Use Queue for parallel eval lanes under explicit
capacity policy.

## Safety Rules

- Private tasks do not become public fixtures without review.
- Eval fixtures cannot contain secrets, provider payloads, or raw customer
  data.
- Model/provider comparison must use equivalent budgets and tool policy.
- Failures caused by policy denials are not counted as runtime crashes.
- Public reports use refs, aggregates, and redacted examples.
- Release gates must name the suite and threshold they enforce.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has Probe benchmark docs, training validation
work, and active W3 issue #4749 for verified traces and first-divergence
evaluation. The terminal-agent README does not yet include an evaluation and
regression audit.

Related open issue anchors:

- #4749 W3 student program.
- #4771 provider peers.
- #4772 MVP exit review.
- #4767 rate-limit rotation proof.
- #4768 overnight unattended proof.

No eval claim should be public without fixture provenance, suite version,
first-divergence reporting, and redaction review.

## Tests

Minimum coverage:

- Run a tiny deterministic eval suite.
- Compare two adapter fixtures.
- Record first divergence.
- Enforce budget and tool-policy equivalence.
- Generate private and public-safe reports.
- Promote a redacted regression fixture.
- Fail a release gate when threshold regresses.
- Verify redaction scans over fixtures and reports.

## Decision

Evaluation should measure behavior under typed conditions and preserve
evidence. It should not be a collection of anecdotal transcripts or
provider-specific success stories.

