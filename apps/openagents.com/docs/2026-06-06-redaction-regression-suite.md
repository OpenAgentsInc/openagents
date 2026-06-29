# Redaction Regression Suite

Date: 2026-06-06

Status: implemented contract note for issue #332 / `OPENAGENTS-085`, extended on
2026-06-07 for issue #453 / `OPENAGENTS-H-016`.

## Purpose

OpenAgents product surface now has shared unsafe redaction fixtures that public, customer, and
agent-facing projection tests can reuse.

The implementation lives in:

- `workers/api/src/redaction-regression-fixtures.ts`; and
- `workers/api/src/redaction-regression.test.ts`.

The suite is intentionally not a snapshot test. It checks exact unsafe fixture
values against representative projection modules so a leak is readable and
actionable.

## Fixture Classes

The shared fixture catalog currently covers:

- secret token-shaped values;
- provider grants;
- callback tokens;
- private prompts;
- payment proofs;
- wallet material;
- private repo refs;
- raw provider payloads;
- raw runner logs; and
- raw ISO timestamps.

The payment fixture catalog now separately covers:

- MDK access-token-shaped values;
- MDK mnemonic and agent-wallet mnemonic-shaped values;
- MDK webhook secret-shaped values;
- private wallet-home paths;
- raw BOLT11 invoices and BOLT12 offers;
- raw payment hashes and payment preimages;
- provider grants;
- Stripe secret-key-shaped values;
- Treasury secret-shaped values;
- raw payout targets;
- exact wallet balance output;
- private checkout refs; and
- private customer/operator identifiers.

## Covered Representative Surfaces

The regression test exercises:

- OpenAgents product surface data policy projections;
- provider placement projections;
- policy exception receipt projections;
- marketplace margin memory projections;
- runner backend projections;
- Blueprint developer package contribution projections;
- buyer payment ledger projections;
- audit export projections;
- Forum public projection decoding; and
- public agent onboarding guidance.

The payment extension also exercises:

- hosted MDK payload scanning;
- L402 response contracts;
- L402 deferred-settlement projections;
- Site payment proof projections;
- Site MDK reconciliation projections;
- Site payment-to-payout bridge projections;
- MDK agent-wallet smoke fixtures;
- self-hosted `mdkd` sidecar option projections;
- payment destination input and redacted destination projections;
- agent spend-cap preview projections;
- buyer payment entitlement policy projections;
- unified payment decision projections;
- Nexus/Treasury payout ledger projections;
- Artanis public report projections; and
- committed public AGENTS/OpenAPI/manifest/onboarding/docs source scans.

## Scanner Tightening From This Issue

The shared fixtures found and fixed older scanner gaps:

- runner backend projections now reject provider grants, raw prompts, private
  repo refs, raw payloads, wallet material, payment proofs, and raw timestamps;
- Forum public projection decoding now rejects callback tokens, provider
  grants, raw prompts, raw payloads, raw runner logs, private repo refs,
  wallet material, payment proofs, and raw timestamps;
- Blueprint developer package contribution projections now reject generic raw
  payload refs; and
- buyer payment ledger projections now reject callback tokens, provider grants,
  private repo refs, wallet material, payment proofs, and raw timestamps in
  string values; and
- audit export projections now reject raw emails, provider grants, provider
  payloads, private repo refs, raw source archives, wallet/payment material,
  payout targets, raw runner logs, and raw timestamps.

## Policy

New public, customer, or agent projection modules should import
`OPENAGENTS_UNSAFE_REDACTION_FIXTURES` and test their reject, redact, or omit
behavior explicitly. Operator projections may expose safe binding or summary
refs, but must never expose raw secrets, provider grants, callback tokens,
wallet material, payment proofs, private repo refs, raw payloads, raw runner
logs, or raw timestamps.

Payment-adjacent modules should additionally use
`OPENAGENTS_PAYMENT_UNSAFE_REDACTION_FIXTURES` when the surface can encounter
payment, checkout, wallet, or payout material. Raw payment destinations can be
accepted as parser input only at explicit payment-destination boundaries; those
projections must still set `rawDestinationProjected: false`, avoid dispatch or
payout authority, and omit exact raw destination values.

Committed public docs and agent API source must not include raw-looking MDK
tokens, mnemonics, webhook secrets, payment hashes, preimages, raw invoices,
Stripe secrets, webhook signing secrets, private wallet paths, or exact wallet
balances. Use placeholders and redacted refs instead.

## Tests

`workers/api/src/redaction-regression.test.ts` covers:

- shared fixture rejection at policy boundaries;
- exact fixture omission from representative projections;
- audit export projection redaction;
- Forum public projection rejection for unsafe artifact and receipt refs; and
- public agent onboarding document safety;
- payment-fixture detection across payment-facing projections;
- payment-destination raw-input reject/redact behavior; and
- committed public docs/API source scans for raw payment secret patterns.
