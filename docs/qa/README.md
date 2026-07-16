# QA execution and retained evidence

This directory contains QA harness implementation notes, operational runbooks,
oracle descriptions, and retained public-safe evidence. It does not own product
intent or verification intent; those live in ProductSpec and AssuranceSpec.

Most files here describe the former Khala Code Desktop system under
`clients/khala-code-desktop`. That client is frozen migration and contract
source material. A file's `implemented` status applies to the dated Khala Code
system and is not current release evidence for `apps/openagents-desktop`.

The existing flat paths are intentionally preserved because scripts, tests,
reports, and historical documents dereference them. Consolidation here means
one honest navigation/status index, not link-breaking file churn.

## Current entry points

- [QA-1 six-lane swarm — first current-main/production run](./2026-07-16-six-lane-swarm/README.md)
- [QA Observer execution loop (QA-2, #8907)](./observer/README.md)
- [Desktop visual baseline gate (QA-3, #8908)](./2026-07-16-desktop-visual-baselines.md)
- [Live QA board pixel proof (QA-4, #8909)](./2026-07-16-live-qa-board/README.md)
- [Independent verifier — no agent accepts its own work (QA-5, #8910)](./verifier/README.md)
- [AssuranceSpec and the MVP dogfood plan](../assurance/README.md)
- [ProductSpec Evidence Loop boundary](../assurance/PRODUCTSPEC_EVIDENCE_LOOP.md)
- [Current MVP definition](../mvp/README.md)
- [Active sequencing](../sol/MASTER_ROADMAP.md)
- [QA Runner runbook](../../apps/qa-runner/RUNBOOK.md)
- [QA Runner capability README](../../apps/qa-runner/README.md)

## Retained Khala Code QA system

### Loop, status, and service projection

- [Nightly matrix](./khala-code-nightly-matrix.md)
- [Status surface](./khala-code-qa-status-surface.md)
- [Flake ledger](./khala-code-flake-quarantine-ledger.json)
- [QA Swarm standing engagement](./qa-swarm-khala-code-standing-engagement.md)

### Corpus and deterministic oracles

- [Architecture scan](./khala-code-architecture-scan.md)
- [Mechanical corpus](./khala-code-mechanical-corpus.md)
- [ThreadItem coverage](./khala-code-thread-item-coverage.md)
- [Error-state corpus](./khala-code-error-state-corpus.md)
- [Cross-mode consistency](./khala-code-cross-mode-consistency.md)
- [Console oracle](./khala-code-mode-d-console-oracle.md)
- [Explore, distill, regress](./khala-code-explore-distill-regress.md)

### Performance and lifecycle

- [Metrics bridge](./khala-code-qa-metrics-bridge.md)
- [Latency budgets](./khala-code-latency-budgets.md)
- [Profiling sweep](./khala-code-lag-profiling-sweep.md)
- [Offender burn-down](./khala-code-lag-offender-burndown.md)
- [Trend regressions](./khala-code-perf-trend-regressions.md)
- [Memory and zombie oracle](./khala-code-memory-zombie-oracle.md)

### Native and visual

- [Visual smoke gate](./khala-code-visual-smoke-gate.md)
- [Visual baselines](./khala-code-visual-baselines.md)
- [Packaged native AX runbook](./khala-code-packaged-native-ax-runbook.md)
- [Flagship demo](./khala-code-flagship-demo.md)

## Historical research and direction

- [Autonomous QA dogfood and every-push design](./2026-06-25-qa-agent-khala-dogfood-and-qa-on-every-push.md)
- [Causely autonomous-remediation note](./2026-06-12-causely-ops-agents-autonomous-remediation.md)
- [Khala Code QA framework design](../fable/2026-07-01-khala-code-desktop-qa-framework-design.md)
- [Historical QA roadmap](../fable/ROADMAP_QA.md)
- [QA Swarm product plan](../fable/2026-07-02-qa-swarm-product-plan.md)
- [Seam-testing audit](../fable/2026-07-06-seam-testing-audit-qa-swarm-gaps.md)
- [Behavior contracts and customer invariants](../fable/2026-07-03-behavior-contracts-and-customer-invariants.md)
- [Deterministic environment testing](../testing/2026-06-22-deterministic-environment-testing.md)
- [Cloudflare artifacts research](../research/2026-06-24-cloudflare-artifacts-for-autonomous-qa.md)

These are source material, not current sequencing. `docs/sol/MASTER_ROADMAP.md`
and live issue state own current work.

## Relationship to AssuranceSpec

AssuranceSpec declares reviewed proof intent for one exact ProductSpec. This
directory contains some of the harnesses, oracles, runbooks, and historical
calibration examples an Assurance Spec may reference. It does not own the
AssuranceSpec format, admission policy, generated Manifest, or current result
projection.

Do not infer current product readiness from a historical green result. Current
claims require an exact target, revision, environment, dependency set, and
fresh receipt.
