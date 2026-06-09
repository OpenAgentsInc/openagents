# oa-workroomd Sidecar Contract

Date: 2026-06-06

Status: implemented contract note for issue #336 / `OPENAGENTS-RUST-003`.

## Purpose

OpenAgents product surface now has a schema-first `oa-workroomd` sidecar session contract.

The implementation lives in
`workers/api/src/oa-workroomd-sidecar-contract.ts`.

This is a contract and projection layer only. It does not launch a daemon,
start Codex/OpenCode, resolve real credentials, upload files, cancel a live
run, archive a workspace, or destroy a local directory.

## Session Model

`OpenAgentsWorkroomdSessionRecord` models:

- workroom daemon session refs;
- assignment intake refs;
- node and daemon refs;
- runtime refs;
- workspace refs;
- grant refs and grant-resolution refs;
- lifecycle event refs;
- artifact manifest refs;
- public and private artifact refs;
- cancellation refs;
- closeout receipt refs;
- failure receipt refs;
- replay/idempotency refs;
- correlation refs;
- route refs;
- source-authority refs;
- policy refs;
- closeout caveat refs; and
- audit evidence refs.

The contract explicitly allows grant refs, not raw credentials. A record can
name `auth_grant.*` or `github_write_grant.*` style refs, but raw auth JSON,
provider payloads, OAuth tokens, cookies, bearer tokens, and API keys are
rejected.

## Lifecycle States

The session status enum covers:

- creating;
- ready;
- running;
- needs context;
- awaiting review;
- closeout ready;
- cancelling;
- cancelled;
- failed;
- closed;
- archived; and
- destroyed.

Cancellation, closeout, and archive/destroy are separate states. Archive and
destroy states do not imply that audit evidence disappeared. The helper
`openAgentsWorkroomdSessionPreservesAuditEvidence` checks that audit evidence
refs remain attached after closeout, failure, archive, or destroy records.

## Projection And Redaction

Public, customer, and agent projections hide grant refs and grant-resolution
refs. Operator/private projections can show safe grant refs so an operator can
debug authority resolution without seeing raw credential material.

All projections use friendly created/updated labels instead of raw timestamps.

The contract rejects:

- raw credentials;
- provider/account auth material;
- provider grants, provider tokens, and provider payloads;
- local paths;
- raw logs;
- raw source archives;
- raw prompts and raw payloads;
- private repo refs;
- wallet/payment material;
- payout targets; and
- raw timestamps.

## Tests

`workers/api/src/oa-workroomd-sidecar-contract.test.ts` covers:

- schema/projection decoding;
- public grant-ref redaction;
- operator-safe grant refs;
- closeout-ready detection;
- cancellation/archive/destroy audit-evidence preservation; and
- unsafe credential, path, log, source archive, private repo, wallet/payment,
  and timestamp rejection.
