# Blueprint Program Run Evidence-Only Guardrails v1

Issue: OPENAGENTS-BP-007 / #227

This note records the service guard that keeps Blueprint Program Runs as
evidence, not authority. The source of truth is
`workers/api/src/blueprint/services/program-run-authority.ts`.

## Denied Direct Effects

Program Runs cannot directly:

- deploy;
- send email;
- create pull requests;
- spend money;
- mutate source-backed facts;
- upgrade public claims.

Those actions must route through future approval-gated Action Submissions.

## Enforcement Shape

The service exposes:

- `assertProgramRunEvidenceOnly`, which rejects any Program Run record that
  carries write-authority flags;
- `denyProgramRunDirectEffect`, which always denies a direct-effect request from
  Program Run authority and names the attempted effect kind.

This is intentionally one-way. Program Runs may produce evidence, decisions, and
recommendations, but cannot become deploy/email/payment/PR/source authority.
