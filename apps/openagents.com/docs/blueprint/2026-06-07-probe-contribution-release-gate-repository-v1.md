# Probe Contribution Release-Gate Repository v1

Date: 2026-06-07

Status: implemented for issue #498.

## Purpose

`blueprint_probe_contributions` stores Probe-originated Blueprint contribution
drafts and promoted contribution refs without granting Probe direct runtime
authority. The repository is the OpenAgents product surface-side ledger for the path from Probe
dogfood contribution to release-gated production ref.

The implementation lives in:

- `workers/api/migrations/0133_blueprint_probe_contributions.sql`
- `workers/api/src/blueprint/repositories/probe-contributions.ts`
- `workers/api/src/blueprint-probe-contribution-routes.ts`

## Stored Shape

Each row records:

- contribution kind, status, and review status;
- candidate and production runtime eligibility booleans;
- release gate readiness;
- blocker refs;
- release gate refs, fixture refs, retained failure refs, and target refs;
- the normalized signature or developer-package contribution record;
- a safe projection; and
- idempotency, creation, update, and archive metadata.

Target refs include Program Type, Program Signature, Module Version, backend
projection adapter, context package, outcome template, tool package, and UI
binding refs.

## Runtime Authority Boundary

Probe contributions cannot carry runtime authority. The repository rejects
contributions that can execute, dispatch runtime, deploy, spend, send email,
mutate repositories, post publicly, create Sites, or change public claims.

Production runtime eligibility is distinct from release-gate entry. A
contribution can be:

- release-gate ready when it is approved for release-gate review and still has
  no promotion ref; or
- production-ready when it is already promoted and still has approved review,
  promotion refs, target refs, release gate refs, fixture refs, retained
  failure refs, no rejection, no runtime authority, and no self-promotion.

The first state lets operators review a candidate. The second lets future Probe
assignment and registry projections consume the promoted ref. Neither state
allows Probe to promote itself.

## Redaction Boundary

The repository validates ids, idempotency keys, target refs, release gate refs,
fixture refs, retained failure refs, dogfood refs, and metadata as safe refs.
It rejects private material such as raw prompts, runner logs, source archives,
provider payloads, provider grants, credentials, wallet material, private
payment values, private customer data, callback tokens, private repos, and raw
timestamps.

Projection data is stored only after the signature or developer-package
projection service filters unsafe refs.

## Tests

Regression coverage lives in:

- `workers/api/src/blueprint/repositories/probe-contributions.test.ts`
- `workers/api/src/blueprint-probe-contribution-routes.test.ts`
- `workers/api/src/blueprint/services/developer-package-contribution.test.ts`
- `workers/api/src/blueprint/exports/contract-export.test.ts`
- `workers/api/src/blueprint-routes.test.ts`
