# L402 Response And Error Contract

Issue: #293 / OPENAGENTS-H-005

Date: 2026-06-06

## Purpose

OpenAgents needs one shared HTTP response contract for payment-required and
payment-credential failures before individual routes start returning L402
challenges. Routes must distinguish economic recovery from auth, scope, safety,
abuse, private authority, manual review, malformed credential, replay, resource
mismatch, and amount mismatch states.

The implementation lives in `workers/api/src/l402-response-contract.ts` with
tests in `workers/api/src/l402-response-contract.test.ts`.

## Response Families

`402 Payment Required`

Used only when the payment policy decision is `recoverable`. The response can
include:

- safe challenge ref;
- product ID;
- endpoint ref;
- amount and spend cap;
- expiry;
- entitlement scope refs;
- docs refs;
- retry/action refs;
- header refs for future `WWW-Authenticate: L402` and `X-OpenAgents-L402`
  behavior.

`401 Unauthorized`

Used for missing auth or failed payment credentials:

- malformed credential;
- invalid signature/proof;
- missing proof;
- expired credential;
- consumed or replayed credential;
- resource mismatch;
- amount mismatch.

`403 Forbidden`

Used for non-payment failures:

- scope missing;
- safety denied;
- abuse denied;
- private authority denied;
- provider capacity unavailable;
- manual review required.

These states intentionally do not include challenge details or pay-to-bypass
instructions.

## Integration Points

The contract integrates at the type level with:

- #289 payment limit policy decisions;
- #290 paid endpoint product records;
- #291 buyer-side payment challenge records;
- #292 L402 credential verification results.

It does not wire every route yet. Future route work should call these builders
after auth, policy, catalog, ledger, and credential checks.

## Redaction Boundary

All response builders reject raw invoices, preimages, wallet secrets, MDK
tokens, private keys, bearer tokens, customer private data, raw payment
payloads, raw prompts, raw runner logs, source archives, provider grants/tokens,
and secret-shaped material.

Public, customer, agent, and operator response projections use refs only. A
non-economic denial never returns payment challenge or recovery-action refs.

## Verification

- `bun run --cwd workers/api test -- src/l402-response-contract.test.ts`
- `bun run --cwd workers/api typecheck`
