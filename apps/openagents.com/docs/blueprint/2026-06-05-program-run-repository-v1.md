# Blueprint Program Run Repository v1

Issue: OPENAGENTS-BP-006 / #226

This note records the first OpenAgents product surface-owned Blueprint Program Run persistence slice.
The source of truth is:

- `workers/api/src/blueprint/schemas/program-run.ts`
- `workers/api/src/blueprint/repositories/program-runs.ts`
- `workers/api/migrations/0100_blueprint_program_runs.sql`

## Purpose

Program Runs are evidence records. They record a typed decision or output from a
Program Signature and Module Version, but they do not authorize writes.

The v1 record stores:

- actor and purpose refs;
- Program Type, Program Signature, and Module Version refs;
- input snapshot hash;
- typed output JSON;
- confidence;
- route and cost refs;
- latency;
- evidence refs;
- receipt refs;
- metadata;
- authority boundary flags.

## Authority Boundary

Every v1 Program Run is `evidence_only` with direct mutation disabled. The row
also stores explicit guardrail booleans:

- no deploy;
- no email;
- no spend;
- no source mutation.

Deploys, emails, PR creation, payment activity, source mutation, public claim
upgrades, and legal-sensitive commitments remain future approval-gated Action
Submissions.

## Current Limits

The repository is now reachable through the Probe evidence intake route at
`POST /api/blueprint/program-runs`. Program Type, Signature, and Module Version
refs are still matched against the seeded Blueprint registry, rather than a
fully persisted registry table. Public/customer projections remain separate
roadmap work; the current route returns only operator-safe Program Run detail
projection data and keeps raw typed output plus metadata in the repository.
