# Native Event And Receipt Contract Registry

Date: 2026-06-06

Status: implemented contract note for issue #334 / `OPENAGENTS-RUST-001`.

## Purpose

OpenAgents product surface now has a seed contract registry for native/Rust-adjacent consumers.

The implementation lives in
`workers/api/src/native-contract-registry.ts`.

This is a schema and projection contract only. It does not launch a daemon,
connect to a machine, dispatch a workroom, spend bitcoin, pay a provider,
upload an artifact, or mutate any runtime state.

## Consumers

The registry currently covers:

- `ai_agent`;
- `openagents_worker`;
- `oa_node`;
- `oa_workroomd`;
- `probe`;
- `psionic`;
- `pylon`;
- `nexus`; and
- `treasury`.

These are consumer identifiers for contracts and fixtures. They are not proof
that the corresponding runtime is live.

## Ref Kinds

The seed registry includes entries for:

- assignment refs;
- heartbeat refs;
- lifecycle event refs;
- artifact refs;
- receipt refs;
- route refs;
- capability refs;
- redaction refs; and
- policy refs.

Each entry carries:

- producer refs;
- consumer refs;
- event or receipt refs;
- payload and receipt schema refs;
- correlation refs;
- idempotency refs;
- source-authority refs;
- policy refs;
- redaction policy refs;
- caveat refs;
- privacy policy; and
- stability.

## Authority Boundary

The registry distinguishes:

- `evidence_only`;
- `approval_required_action`; and
- `executed_action_receipt`.

Most entries are evidence-only. Assignment contracts are modeled as
approval-required action intents. Receipt contracts are modeled as executed
action receipt references.

This distinction is for downstream safety checks. It does not grant the right
to dispatch, deploy, mutate source, send email, spend, or settle payout.

## Projection And Redaction

Internal registry records carry ISO timestamps so audit code can sort and
compare them. Projections expose friendly display labels such as "10 minutes
ago" and never raw timestamps.

Public, customer, and agent projections hide operator/private route,
capability, policy, source, and workroom refs. Operator/private projections can
show broader safe refs but still reject raw secrets and raw payload material.

The contract rejects refs containing:

- secrets, bearer tokens, callback tokens, cookies, OAuth material, and API
  keys;
- provider grants, provider tokens, provider accounts when unsafe, and raw
  provider payloads;
- private repo material;
- wallet material, payment proofs, payment hashes, preimages, invoices, and
  payout targets;
- raw auth payloads;
- raw runner logs;
- raw source archives;
- raw prompts, raw webhooks, raw payloads, and raw emails; and
- raw timestamps.

## Tests

`workers/api/src/native-contract-registry.test.ts` covers:

- schema decoding for the registry seed;
- consumer and ref-kind coverage;
- evidence-only versus authority-boundary classification;
- public-safe projection with friendly times;
- no raw timestamp projection; and
- unsafe fixture rejection.
