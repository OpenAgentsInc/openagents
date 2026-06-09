# L402 Payment Header Contract

Issue: #294 / OPENAGENTS-H-006

Date: 2026-06-06

## Purpose

OpenAgents needs a standard header contract for paid retries before individual
routes start enforcing L402 challenges. Agent clients often already need
`Authorization: Bearer ...` for identity, while generic payment clients expect
`Authorization: L402 ...` or legacy `Authorization: LSAT ...` compatibility.

The implementation lives in `workers/api/src/l402-payment-headers.ts` with
tests in `workers/api/src/l402-payment-headers.test.ts`.

## Accepted Header Shapes

`WWW-Authenticate: L402 ...`

Routes can format a public-safe challenge header with challenge, product,
endpoint, amount, asset, denomination, expiry, and docs refs. The formatter
rejects raw invoices, preimages, wallet material, bearer tokens, customer
private data, provider tokens, and secret-shaped values.

`Authorization: L402 <credential>:<proof-ref>`

Generic clients can submit an OpenAgents L402 credential and a public-safe
payment proof ref in the standard authorization header. The credential must use
the `oa-l402-v1.` prefix.

`Authorization: LSAT <credential>:<proof-ref>`

Legacy LSAT-compatible clients can use the same credential/proof-ref pair with
the `LSAT` scheme. The parser labels this separately so route code can preserve
compatibility metrics.

`Authorization: Bearer <agent-token>` plus
`X-OpenAgents-L402: <credential>:<proof-ref>`

Agent clients can keep bearer auth intact and send payment proof separately.
This prevents paid retries from replacing the agent identity token.

## Parse States

The parser returns typed states for:

- missing payment header;
- bearer auth without payment proof;
- `X-OpenAgents-L402` payment proof with optional bearer auth;
- standard L402 authorization;
- legacy LSAT authorization;
- unsupported authorization scheme;
- malformed payment credential/proof pair;
- collision between non-bearer `Authorization` payment material and
  `X-OpenAgents-L402`.

Route code should treat bearer-only as an auth state, not a payment proof.
Route code should treat collision as invalid so a caller cannot accidentally or
ambiguously present two payment credentials.

## Projection Boundary

Parse results can carry the raw L402 credential because verifier code needs it.
Public projections never include that credential. Public projections also hide
the proof ref; agent, customer, and operator projections can include the proof
ref because it is a redacted ref, not raw payment material.

The projection helper rejects secret-shaped values and is covered by regression
tests. This is the boundary routes should use for logs, public proof, customer
status surfaces, and agent-readable diagnostics.

## Verification

- `bun run --cwd workers/api test -- src/l402-payment-headers.test.ts`
- `bun run --cwd workers/api typecheck`
