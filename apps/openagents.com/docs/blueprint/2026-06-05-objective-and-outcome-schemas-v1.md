# Blueprint Objective and Outcome Schemas v1

Issue: OPENAGENTS-BP-003 / #223

This note records the first OpenAgents product surface-owned Blueprint Objective and Outcome schema
slice. The source of truth is `workers/api/src/blueprint/schemas/objective.ts`.

## Purpose

Objective schemas give Autopilot, Adjutant, Omni workrooms, and future Pylons a
typed way to describe what a run is trying to accomplish before any tool is
allowed to write. Outcome linkage then connects that run to an Omni accepted
outcome contract when evidence is ready for review or acceptance.

This keeps the Blueprint layer evidence-oriented:

- Objective Types define intent, allowed surfaces, policy refs, budgets, risks,
  metrics, reward refs, utility refs, and release gates.
- Objective Runs bind a concrete run to an Objective Type, work kind, workroom,
  program run, release gate refs, and outcome evidence refs.
- Accepted Outcome links point at Omni accepted outcome contracts and public
  receipt refs instead of storing raw customer, provider, email, payment, or run
  log material.

## Modeled Contracts

- `BlueprintObjectiveType`
- `BlueprintObjectiveRun`
- `BlueprintAcceptedOutcomeLink`
- `BlueprintObjectiveMetricRef`
- `BlueprintObjectiveGuardrailPolicy`
- `BlueprintObjectiveBudgetPolicy`
- `BlueprintObjectiveRiskPolicy`
- `BlueprintObjectiveReleaseGate`

The schema also defines bounded enums for allowed surfaces, policy severity,
budget kind, budget enforcement, risk kind, release gate kind, and run status.

## Compatibility With Omni

The outcome link reuses Omni accepted outcome work kinds and acceptance states.
That gives Blueprint a typed bridge into existing site, coding, adjustment,
existing-project-import, business, and legal-sensitive fulfillment records
without making Program Runs the write authority.

## Current Limits

This is a schema boundary only. Persistence, JSON Schema export, OpenAPI export,
and approval-gated write services are intentionally later Blueprint issues.
