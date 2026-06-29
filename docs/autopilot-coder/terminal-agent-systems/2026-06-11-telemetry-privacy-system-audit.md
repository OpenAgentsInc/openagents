# Telemetry And Privacy System Audit

Date: 2026-06-11

This is system #47 from the Bun/Effect terminal-agent systems list. It defines
how OpenAgents should measure runtime health, usage, errors, cost, and product
quality without collecting raw private work.

## Target

Build a privacy-first telemetry system that emits metrics and aggregate
diagnostics from terminal-agent runs while keeping raw prompts, private repo
content, shell output, provider payloads, wallet material, and customer data
out of telemetry.

## User-Visible Capability

Users should be able to:

- See whether telemetry is enabled.
- Choose local-only, private account, team, or product-improvement modes.
- Export a redacted diagnostic bundle.
- Understand which categories are collected.
- Disable nonessential telemetry.
- Keep sensitive work out of product analytics.
- See cost and token usage for their own runs.

Telemetry defaults should be conservative. Operational receipts can exist
without product analytics.

## Telemetry Classes

Recommended classes:

- Local diagnostics: never leaves device.
- Account usage: visible to the user and team under billing policy.
- Product metrics: aggregate and private-data-free.
- Health events: service availability, adapter status, error class.
- Cost events: tokens, provider cost estimates, budget stops.
- Safety events: denials, blocked unsafe projections, redaction hits.

Each event class should declare retention, visibility, exportability, and
whether user opt-out applies.

## Bun/Effect Boundary

Use Effect services for:

- `TelemetryPolicyService`: resolves mode and collection rules.
- `TelemetryEmitterService`: records structured telemetry events.
- `UsageAccountingService`: tracks tokens, cost, and budget.
- `PrivacyFilterService`: scans event payloads before emission.
- `TelemetryProjectionService`: shows user and operator-safe summaries.
- `TelemetryExportService`: emits local diagnostic bundles.

Use Schema for telemetry event classes and payloads. Use Queue for batching and
backpressure. Use Schedule for flush and retry. Use Layer to swap local,
account, and disabled sinks.

## Safety Rules

- No raw prompts, private code, raw command output, provider payloads, tokens,
  invoices, wallet data, or customer records in telemetry.
- Telemetry events use refs, counts, durations, and coarse classes.
- User opt-out disables product-improvement telemetry.
- Billing and receipt events remain governed by payment and authority policy,
  not product analytics preferences.
- Telemetry sinks are declared and visible.
- Failed telemetry delivery cannot fail the main agent run.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has Pylon host inventory telemetry, token usage
accounting, redaction checks, and product-promise projection policy in adjacent
surfaces. The terminal-agent README does not yet include a telemetry/privacy
system audit.

Related open issue anchors:

- #4770 team budgets and spend-to-evidence join.
- #4766 account-pool dashboard.
- #4767 rate-limit rotation proof smoke.
- #4768 overnight unattended proof smoke.
- #4772 MVP exit review.

No telemetry claim should be broad until collection classes, opt-out behavior,
redaction scans, and retention rules are tested.

## Tests

Minimum coverage:

- Emit each telemetry class through local, disabled, and aggregate sinks.
- Block payloads containing forbidden private material.
- Verify opt-out disables product telemetry but not required local receipts.
- Preserve cost accounting per run and team budget.
- Drop telemetry on sink failure without disrupting runtime.
- Redact support bundles.
- Enforce retention policy per class.
- Show telemetry mode in diagnostics.

## Decision

Telemetry should measure runtime behavior with refs and aggregates. It should
not become a shadow transcript, prompt archive, or private-work collection
system.

