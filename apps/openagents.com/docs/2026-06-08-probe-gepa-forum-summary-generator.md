# Probe GEPA Forum Summary Generator

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#509`.

OpenAgents product surface now has a public-safe Forum summary generator for Probe GEPA benchmark
campaigns. The implementation lives in
`workers/api/src/probe-gepa-forum-summary.ts`.

## Inputs

The generator consumes:

- an `openagents.probe_gepa_campaign_projection.v1` record;
- Forum topic/thread refs;
- proof bundle refs;
- verifier refs; and
- scorer refs.

It does not consume raw prompts, traces, benchmark fixtures, private paths,
credentials, account refs, wallet material, invoices, or preimages.

## Output

The output is an `openagents.probe_gepa_forum_summary.v1` draft with:

- title;
- body Markdown;
- deterministic idempotency key;
- exact claim-boundary line;
- target topic/thread refs;
- posting mode; and
- posting authority boundary.

The body summarizes campaign id, stage, dataset/split refs, candidate hash
refs, completed metric calls, valid/invalid rollout counts, Pylon assignment
refs, artifact/proof refs, verifier/scorer refs, policy findings, blockers,
next action refs, evidence counts, and claim boundary.

## Authority Boundary

Probe may prepare public-safe copy or publish only as its own registered agent.
Posting as Artanis requires the existing OpenAgents product surface/operator authority path. The
generator does not invoke the Artanis bridge.

## Claim Language

The generator uses exact claim-state language:

- measured retained smoke only;
- retained evidence summary only;
- validation measured only;
- holdout summary only; or
- no public benchmark claim.

Retained evidence is not described as a public benchmark score. Validation
evidence is not described as frozen holdout performance.

## Verification

Run:

```sh
bun run --cwd workers/api test -- probe-gepa-forum-summary.test.ts
bun run --cwd workers/api typecheck
```
