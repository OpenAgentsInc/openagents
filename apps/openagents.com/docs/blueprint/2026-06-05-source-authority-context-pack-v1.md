# Blueprint Source Authority And Context Pack v1

Issue: OPENAGENTS-BP-009 / #229

This note records the typed Source Authority and Context Pack model. The source
of truth is `workers/api/src/blueprint/schemas/source-context.ts`.

## Purpose

Context Packs define what an agent run may use as context. They can narrow the
available material, but they cannot widen an actor's underlying authority.

Source Authority records capture:

- source ref;
- source kind;
- freshness;
- consent state;
- confidence;
- data classification;
- trust tier;
- included/excluded state;
- public/customer-safe projection flags;
- public summary ref.

The first source kinds cover orders, Exa briefs, repositories, emails,
artifacts, customer assets, and generated summaries.

## Projection Rules

Public and customer projections are filtered through Omni data classification
rules plus explicit public/customer-safe flags. Raw private provider, account,
email, payment, and secret-bearing material must remain excluded from public and
customer surfaces.

## Current Limits

This issue defines contracts and projection helpers only. Persistence, source
pack assembly, route integration, Exa freshness refresh, and UI display are
separate roadmap issues.
