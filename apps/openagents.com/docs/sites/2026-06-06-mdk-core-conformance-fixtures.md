# MDK Core Conformance Fixtures

Issue: #297 / OPENAGENTS-H-007B

Date: 2026-06-06

## Purpose

OpenAgents product surface now has public-safe conformance fixtures for the MDK checkout and L402
behaviors it intends to rely on. The fixture set keeps the Worker-native
contracts compatible with useful MDK semantics without making live MDK network
calls, publishing real payment credentials, or settling payouts.

The executable fixture catalog lives in
`workers/api/src/mdk-core-conformance-fixtures.ts` with tests in
`workers/api/src/mdk-core-conformance-fixtures.test.ts`.

## Fixture Coverage

Implemented fixtures:

- amount checkout creation;
- product checkout creation;
- customer field normalization;
- metadata limits;
- signed checkout URL creation and verification with Web Crypto;
- safe Site-local checkout path handling;
- sandbox flag preservation;
- stale hosted challenge behavior;
- typed error envelopes.

Adapted fixtures:

- L402 token parsing through OpenAgents product surface's Worker-compatible credential service;
- price re-check through credential verification expectations;
- proof verification boundary through redacted proof refs instead of raw
  preimages.

Deferred behaviors:

- live MDK checkout network calls;
- direct wallet/node state;
- provider payout settlement;
- production invoice minting;
- customer-owned MDK account mode.

## Redaction Rules

Fixtures use synthetic refs and safe metadata only. They do not include raw
production invoices, payment preimages, wallet mnemonics, MDK access tokens,
webhook secrets, provider grants, customer private data, raw prompts, raw
runner logs, or live payment payloads.

Fixture names and docs use "bitcoin" except where the exact schema
denomination `bitcoin_millisatoshi` is required.

## Verification

The conformance tests execute:

- human amount and product checkout preparation;
- redacted projections;
- route dispatch;
- metadata limit failures;
- signed URL validity and tamper checks;
- stale hosted challenge errors;
- L402 valid, malformed, amount-mismatch, and proof-missing states;
- L402 error response redaction.

## Commands

- `bun run --cwd workers/api test -- src/mdk-core-conformance-fixtures.test.ts`
- `bun run --cwd workers/api typecheck`
