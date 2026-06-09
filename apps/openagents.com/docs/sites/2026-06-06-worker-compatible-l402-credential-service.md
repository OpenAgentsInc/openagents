# Worker-Compatible L402 Credential Service

Issue: #292 / OPENAGENTS-H-004

Date: 2026-06-06

## Purpose

OpenAgents needs an L402-compatible credential contract before recoverable
economic limits can return payment challenges that agents and generated Sites
can satisfy. This slice adds a Worker-compatible credential service without
requiring live MDK credentials, creating invoices, settling payments, or
granting provider payout claims.

The implementation lives in
`workers/api/src/l402-credential-service.ts` with tests in
`workers/api/src/l402-credential-service.test.ts`.

## Credential Payload

The payload version is `oa-l402-v1` and binds:

- challenge ref;
- credential ref;
- product ID;
- endpoint ref;
- method;
- path;
- optional request-body digest;
- amount, asset, and denomination;
- expiry;
- payment hash ref;
- entitlement scope refs;
- idempotency key hash;
- replay nonce ref;
- issue timestamp.

The credential string is:

```text
oa-l402-v1.<base64url canonical payload>.<base64url signature>
```

The canonical payload is stable JSON so the signature does not depend on object
insertion order.

## Signing Boundary

`makeOpenAgentsL402HmacSigningBoundary` uses `crypto.subtle` HMAC-SHA-256, so it
works in Workers and test runtimes. It accepts secret key material only as an
input to the signing boundary and never includes that material in credential
payloads, projections, docs, or issue comments.

The boundary is intentionally replaceable. Later hosted MDK or secret-backed
signing can keep the same payload, mint, verify, and projection contracts.

## Verification Results

`verifyOpenAgentsL402Credential` returns typed results:

- `valid`
- `malformed`
- `signature_invalid`
- `expired`
- `amount_mismatch`
- `resource_mismatch`
- `consumed_or_replayed`
- `proof_missing`

Verification checks signature, optional proof presence, expiry, consumed/replay
refs, method, path, product, endpoint, challenge, optional request digest,
entitlement scopes, and amount.

## Integration With Payment Contracts

The service integrates at the type level with:

- #289 payment limit policy, by giving recoverable economic limits a concrete
  future credential result;
- #290 paid endpoint product catalog, by binding product, endpoint, method,
  path, amount, and entitlement refs;
- #291 buyer-side payment ledger, by deriving payloads from
  `BuyerPaymentChallengeRecord` and using the shared amount schema.

The service does not store credentials or consume entitlements by itself. The
buyer-side ledger remains the replay/entitlement authority. Future route
middleware should verify a credential, then ask the ledger whether the
credential or replay nonce has already been consumed.

## Redaction Boundary

The service rejects raw invoices, preimages, wallet secrets, MDK tokens,
private keys, bearer tokens, customer private data, provider grants/tokens,
raw payment payloads, raw prompts, raw runner logs, source archives, and other
secret-shaped material.

`projectOpenAgentsL402Credential` emits safe public, customer, agent, and
operator projections. Public projections omit payment hash refs, replay nonce
refs, signer refs, and the credential string. Customer and agent projections can
see the redacted payment hash ref. Operator projections can also see signer and
replay nonce refs. No projection includes the signed credential string.

## Non-Goals

This slice does not:

- create real MDK invoices;
- verify real payment preimages;
- settle payments;
- grant provider payout claims;
- bypass safety, auth, moderation, private authority, or manual-review gates.

## Verification

- `bun run --cwd workers/api test -- src/l402-credential-service.test.ts`
- `bun run --cwd workers/api typecheck`
