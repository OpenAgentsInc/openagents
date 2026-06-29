# Blueprint Simulation Branch And Scenario Fork Records v1

Issue: OPENAGENTS-BP-013 / #233

This note records the typed Simulation Branch and Scenario Fork model. The
source of truth is `workers/api/src/blueprint/schemas/simulation.ts`.

## Purpose

Simulation Branches let risky workflows replay isolated state before production
rollout. The first purposes cover risky workflows, migrations, destructive
action suites, and autonomy promotion.

Scenario Forks separate:

- base production snapshot refs;
- simulated fork state refs;
- simulated effect refs;
- production effect refs.

## No Production Effects

The v1 effect isolation mode is `simulated_only`. Projection helpers expose
whether a branch has production-effect leakage. A scenario fork with any
production effect refs is not safe for rollout.

## Current Limits

This is a schema and projection layer only. Persistence, replay execution,
state-diff rendering, migration runners, destructive-action simulators, and
promotion gates are separate roadmap issues.
