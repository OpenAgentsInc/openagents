# Testing And Smoke System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #51 from the Bun/Effect terminal-agent systems list. It defines
the layered test and smoke discipline needed before terminal-agent capability
claims can be treated as real.

## Target

Build a testing system that separates unit tests, contract fixtures,
integration tests, CI-safe smokes, local-device smokes, staging smokes, and
live smokes.

Each smoke should state what it proves and what it does not prove.

## User-Visible Capability

Users and operators should be able to:

- Run quick local checks before trusting a setup.
- Verify adapters, workspaces, credentials, and provider availability without
  leaking secrets.
- See smoke receipts for release and product claims.
- Distinguish no-spend, paid, local, staging, and live smokes.
- Reproduce failures with redacted fixtures.

## Test Layers

Recommended layers:

- Pure schema and reducer tests.
- Service tests with memory stores.
- Fixture-based tool and workspace tests.
- Adapter contract tests.
- CI-safe smoke tests.
- Local-device smoke tests.
- Staging endpoint smokes.
- Live endpoint smokes with explicit approval.
- Regression fixtures from real failures after redaction.

No live-spend or live-write smoke should run accidentally.

## Bun/Effect Boundary

Use Effect layers to swap:

- Filesystem, shell, provider, and network boundaries.
- Artifact and receipt stores.
- Clock and schedule.
- Telemetry sinks.
- Permission resolvers.
- Model/provider adapters.

Use Schema fixtures for external payloads and event logs. Use Scope for
temporary workspaces. Use tagged errors so smoke output is machine-readable.

## Safety Rules

- Live smokes require explicit flags and policy refs.
- Secrets are supplied by environment or managed bindings, never fixtures.
- Smoke output is public-safe unless explicitly marked private.
- Failure output includes blocker refs, not raw logs by default.
- Paid smokes distinguish payer evidence, accepted work, and settlement.
- CI-safe smokes must not depend on private services.
- Smoke receipts must not overclaim.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has many Pylon, Autopilot, payment, Forum, and
agent-runtime smokes. The terminal-agent README does not yet include a general
testing/smoke system audit.

Related open issue anchors:

- #4767 rate-limit rotation proof smoke.
- #4768 overnight unattended proof smoke.
- #4772 MVP exit review.
- #4777 first live negotiated labor job.
- #4786 MVP ladder.
- #4749 W3 student program evaluation work.

No product promise should turn green on the strength of a smoke unless the
smoke's proof boundary matches the promise exactly.

## Tests

Minimum coverage:

- Run fixture, integration, CI-safe smoke, and live-gated smoke commands.
- Require explicit approval flags for live spend, push, deploy, or settlement.
- Produce public-safe receipts.
- Verify redaction scans across smoke output.
- Classify failure reasons with blocker refs.
- Ensure test layers use fake providers unless live mode is requested.
- Preserve exact command, version, and environment classification.
- Reject stale smoke receipts for freshness-bound claims.

## Decision

Testing and smokes should be a product-claim authority system. Passing tests
prove bounded contracts; they do not automatically prove broader capability
copy.

