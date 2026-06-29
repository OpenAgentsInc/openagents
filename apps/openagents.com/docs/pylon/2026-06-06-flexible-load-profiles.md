# Pylon Flexible-Load Profiles

Date: 2026-06-06

Status: implemented contract note for GitHub issues #324 / `OPENAGENTS-077` and
#364 / `OPENAGENTS-LATE-004`.

## Purpose

OpenAgents needs to know which work classes can pause, resume, checkpoint, or
shift by deadline before those jobs are routed through Pylon/provider capacity
or described as flexible load.

The implementation lives in
`workers/api/src/pylon-flexible-load-profiles.ts`.

## Profile Shape

The v1 profile records:

- work kind and safe Coding/Sites/Omni work-class refs;
- flexibility class;
- interruption tolerance;
- checkpoint cadence and checkpoint policy refs;
- resume requirement and resume policy refs;
- deadline window;
- verification-after-resume policy;
- replay cost;
- power-event eligibility;
- modeled suitability refs;
- measured response refs;
- accepted outcome refs;
- revenue refs; and
- settlement refs.

These fields are intentionally separate. A work class that is modeled as
interruptible is not automatically measured. A measured flexible-load response
is not automatically accepted work. Accepted work is not automatically revenue
or settled payout.

Flexibility class can be:

- `fixed`;
- `deferrable`;
- `interruptible`;
- `preemptible`; or
- `opportunistic`.

The contract rejects incoherent profile claims:

- fixed work cannot claim interruption, checkpoint, or power-event
  eligibility;
- interruptible and preemptible work must declare interruption tolerance; and
- deferrable work cannot use an immediate deadline.

## Claim Boundaries

The projection exposes separate flags:

- `modeledSuitabilityClaimAllowed`;
- `measuredSuitabilityClaimAllowed`;
- `acceptedOutcomeClaimAllowed`;
- `revenueClaimAllowed`; and
- `settlementClaimAllowed`.

Power-event eligibility can be:

- `not_eligible`;
- `operator_review`;
- `eligible_modeled`; or
- `eligible_measured`.

`eligible_modeled` requires modeled suitability refs. `eligible_measured`
requires both modeled suitability refs and measured response refs.

Revenue refs require accepted outcome refs. Settlement refs require accepted
outcome refs and revenue refs.

## Authority Boundary

Issue #364 adds explicit read-only profile authority. A flexible-load profile
cannot:

- assign capacity;
- dispatch a power event;
- launch a runner;
- mutate settlement;
- mutate a work class;
- upgrade a public claim.

The profile is a routing and review input before dispatch automation, not a
dispatch command.

## Evidence Requirements

The contract rejects over-broad claims:

- checkpoint-required work must have a non-`none` checkpoint cadence;
- non-`none` checkpoint cadence must have checkpoint policy refs;
- resumable work must have resume policy refs;
- verification-after-resume must have verification policy refs;
- modeled eligibility must have modeled suitability refs; and
- measured eligibility must have measured response refs.

## Redaction Rules

All projections reject raw provider telemetry, private hardware details, raw
runner logs, wallet material, payment identifiers, payout targets, invoices,
preimages, private keys, mnemonics, customer data, private repo refs, secrets,
and raw timestamps.

Public/customer projections also hide private accepted-outcome, measured,
revenue, settlement, and provider refs. Team projections hide private provider
and settlement refs. Operator projections can see audience-safe internal refs,
but they still cannot contain raw secrets or private machine/payment material.

## Deferred Work

This profile layer is not the flexible-load event ledger. Issue #365 adds the
first event telemetry contract in
`2026-06-06-flexible-load-event-telemetry.md`, covering requested/actual
response, checkpoint/resume refs, lost-work cost, accepted-work impact, and
settlement evidence without dispatch authority. Later work should add:

- actual grid/operator/customer power-event records;
- checkpoint/resume event refs;
- lost-work cost and replay-cost measurements;
- accepted-work impact refs;
- proof bundles that join profile, event, outcome, economics, and settlement
  evidence without exposing private telemetry.

## Tests

`workers/api/src/pylon-flexible-load-profiles.test.ts` covers:

- public-safe profile projection and friendly timestamps;
- flexibility class labels and policy coherence;
- modeled, measured, accepted, revenue, and settlement separation;
- audience redaction;
- false dispatch, runner, settlement, work-class, and public-claim authority;
- required evidence for checkpoint/resume/verification/model/measured states;
  and
- rejection of raw telemetry, hardware, runner, wallet, payment, payout, and
  timestamp material.
