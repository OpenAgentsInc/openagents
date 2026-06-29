# Site Payment Manifest Schema

Issue: #298 / OPENAGENTS-SITES-MDK-001

Date: 2026-06-06

## Purpose

Generated Sites need a source-visible payments contract for checkout products
and paid agent actions. The manifest can declare intent, but it must not carry
MDK credentials, raw payment material, customer private data, provider grants,
or payout claims.

The implementation lives in `workers/api/src/site-payment-manifest.ts` with
tests in `workers/api/src/site-payment-manifest.test.ts`.

## Manifest Shape

The `.openagents/site.json` `payments` block supports:

- hosted provider `openagents_hosted_mdk`;
- checkout products;
- paid actions;
- prices using `usd_cent`, `credit`, or the exact bitcoin denomination
  `bitcoin_millisatoshi`;
- settlement mode: `checkout_only`, `deferred`, or `accepted_work_linked`;
- entitlement scope: `site`, `product`, `path`, `action`, or `account`;
- customer data requirement descriptors;
- clean Site-local checkout paths;
- agent-readable flags;
- sandbox flags;
- public projection state.

## Redaction Rules

The decoder rejects raw invoices, payment preimages, wallet mnemonics, MDK
access tokens, webhook secrets, provider grants, customer private data, raw
prompts, raw runner logs, checkout result query state, absolute checkout URLs,
protocol-relative URLs, and secret-shaped values.

Manifest projections expose only IDs, display refs, checkout paths, prices,
entitlement scope, settlement mode, sandbox state, and agent-readable flags.
They do not expose customer-data requirement details or metadata refs.

## Authority Boundary

The manifest is not payment authority. OpenAgents product surface still owns checkout intent
creation, hosted MDK calls, buyer-side payment evidence, entitlements,
receipts, reconciliation, and public-safe proof.

## Verification

- `bun run --cwd workers/api test -- src/site-payment-manifest.test.ts`
- `bun run --cwd workers/api typecheck`
