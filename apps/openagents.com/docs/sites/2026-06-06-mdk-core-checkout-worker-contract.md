# MDK Core Checkout Worker Contract

Issue: #296 / OPENAGENTS-H-007A

Date: 2026-06-06

## Purpose

OpenAgents product surface needs the useful MoneyDevKit checkout semantics without importing MDK's
Next.js wrappers or exposing MDK implementation details to generated Sites.
This slice ports the core contract ideas into Effect Schema and
Worker-compatible helpers.

The implementation lives in
`workers/api/src/mdk-core-checkout-contract.ts` with tests in
`workers/api/src/mdk-core-checkout-contract.test.ts`.

## Ported Contract Pieces

The Worker contract includes:

- route selection for `create_checkout`, `get_checkout`, and
  `confirm_checkout` from MDK-style `handler`, `route`, or `target` fields;
- amount checkout and product checkout input schemas;
- customer field normalization to camelCase;
- metadata key count, key length, UTF/control-character, size, and
  secret-material validation;
- sandbox flags;
- safe Site-local checkout path sanitation;
- signed checkout URL creation and verification with Web Crypto HMAC;
- redacted prepared-checkout projections;
- a hosted checkout plan schema bridging prepared checkout state to the hosted
  MDK client contract from #295 and optional L402 payloads from #292.

## Source Boundary

The port intentionally does not copy MDK UI, React hooks, Next.js route
handlers, `process.env` reads, Node `crypto`, provider payouts, preview
payment mutation, or direct wallet/node state.

Generated Site source and public manifests must never contain MDK credentials,
wallet mnemonics, raw invoices, payment preimages, webhook secrets, provider
payloads, customer private data, raw prompts, or raw runner logs.

## Checkout Path Rules

Checkout paths default to `/checkout`. Accepted paths must be Site-local paths
that start with `/`. Absolute URLs and protocol-relative URLs fall back to the
default. Query strings and fragments are stripped before signing so return
state does not leak into public URLs.

## Projection Rules

Prepared checkout projections include only:

- mode;
- amount or product refs;
- sanitized checkout path;
- metadata keys;
- customer field keys;
- required customer field names;
- sandbox flag.

Customer values and metadata values are redacted in every projection. MDK
provider state, wallet state, invoices, preimages, and payout claims are not
part of the projection contract.

## Integration Points

The hosted checkout plan schema connects this contract to:

- #295 hosted MDK client requests;
- #292 L402 credential payloads;
- #291 buyer-side payment challenge and amount records through the hosted
  client request shape.

Future route work should use these contracts as the Worker-owned checkout
surface before calling the hosted MDK client.

## Verification

- `bun run --cwd workers/api test -- src/mdk-core-checkout-contract.test.ts`
- `bun run --cwd workers/api typecheck`
