# Blueprint Module Version Schema v1

Issue: OPENAGENTS-BP-005 / #225

This note records the OpenAgents product surface-owned Blueprint Module Version schema slice. The
source of truth is `workers/api/src/blueprint/schemas/module.ts`.

## Purpose

Module Versions are implementation artifacts behind Program Types and Program
Signatures. They can represent deterministic reducers, model prompts, Effect
agent modules, runtime adapters, human-review modules, and optimizer
candidates.

The schema gives each module version explicit:

- module kind;
- lifecycle status;
- release state;
- implementation and artifact refs;
- Program Type and optional Program Signature linkage;
- provenance;
- scorecards;
- release decision;
- rollback and deprecation anchors.

## Release-State Guardrail

Module Versions cannot self-promote into production.

Production requires a release decision with an operator/review decision ref and
release gate ref. Optimizer candidates remain candidates until a separate
approval path promotes them. Rolled-back modules require a rollback target, and
deprecated modules require a deprecation timestamp.

## Current Limits

This is a schema and predicate boundary only. Persistence, optimizer-run records,
release-gate services, promotion APIs, rollback APIs, and Program Registry UI are
separate roadmap issues.
