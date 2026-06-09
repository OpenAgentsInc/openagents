# Marketplace Margin Memory Hooks

Date: 2026-06-06

Status: implemented contract note for issue #328 / `OPENAGENTS-081`.

## Purpose

Marketplace margin memory records let OpenAgents product surface remember which Program Signatures,
modules, tools, sources, packages, providers, reviewers, routes, and work
classes helped produce accepted work.

The implementation lives in
`workers/api/src/marketplace-margin-memory.ts`.

This is an attribution and measurement contract only. It does not rank
marketplace entries, promote modules, route work, settle payouts, or mutate
public claims.

## Record Shape

`MarketplaceMarginMemoryRecord` tracks:

- accepted count and accepted outcome refs;
- rejected count and rejected outcome refs;
- refund count and refunded outcome refs;
- retry count and retry evidence refs;
- review burden score and review burden refs;
- gross-margin evidence refs;
- revenue evidence refs;
- modeled marketplace value refs;
- repeat-buyer signal refs;
- settlement state refs;
- capability, market-memory, Program Signature, Module Version, package,
  tool, source, provider, reviewer, route, caveat, evidence, and work-class
  refs; and
- review state.

Counts and evidence refs are intentionally separate so the system can state
what has been measured without overstating revenue, gross margin, settlement,
repeat-buyer quality, or marketplace value.

## Claim Separation

The projection exposes separate booleans for:

- accepted outcome claims;
- rejected outcome claims;
- refund claims;
- modeled marketplace value claims;
- revenue claims;
- gross-margin claims;
- repeat-buyer claims; and
- settlement claims.

Accepted outcomes do not imply revenue. Revenue does not imply gross margin.
Gross margin requires both revenue and accepted outcome refs. Settlement stays
its own claim so pending payout state cannot be presented as completed payout
evidence.

## Authority Boundary

The only allowed authority boundary is `evidence_only`.

The default authority block denies:

- automatic public rank mutation;
- module promotion;
- payout mutation;
- routing mutation; and
- settlement mutation.

`marketplaceMarginMemoryHasMutationAuthority` detects accidental authority.
Projection rejects records that try to carry mutation authority.

Reviewed, release-gate-ready, or promoted records with accepted outcomes and
evidence can become public rank candidates, but even then
`automaticPublicRankMutationAllowed`, `routingMutationAllowed`,
`modulePromotionAllowed`, `payoutMutationAllowed`, and
`settlementMutationAllowed` remain false.

Draft and unreviewed records cannot become public rank candidates.

## Redaction

Projections reject:

- private customer data;
- raw source archives;
- raw prompts, provider payloads, raw webhook material, and raw runner logs;
- bearer tokens, OAuth material, API keys, provider tokens, and secrets;
- wallet material, Lightning invoices, payment hashes, preimages, and payout
  targets;
- private repo refs; and
- raw timestamps.

Public and customer projections also hide private provider, reviewer, revenue,
settlement, and source refs. Team projections hide private provider and
settlement refs. Operator projections can carry safe internal refs, but still
cannot carry secrets, raw payloads, wallet material, private repo refs, or raw
timestamps.

## Tests

`workers/api/src/marketplace-margin-memory.test.ts` covers:

- schema/projection decoding;
- accepted outcome attribution without mutation authority;
- separation of outcome, modeled value, revenue, gross margin, refund,
  repeat-buyer, and settlement claims;
- blocking draft, unreviewed, and authoritative records from rank or routing
  mutation;
- required evidence refs for counts and economic claims; and
- redaction and rejection of unsafe marketplace memory material.
