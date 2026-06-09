# Signature Marketplace Revenue Gate

Date: 2026-06-08

Issue: [#564](https://github.com/OpenAgentsInc/openagents/issues/564)

## Launch Decision

Plugin and signature marketplace revenue is blocked until OpenAgents product surface can project a
public-safe settled usage event. Existing signature package validation is
read-only; it does not install a package, promote a runtime, list a package,
mutate payment state, or create payout eligibility.

The launch predicate is:

1. Package validation refs exist.
2. Package refs exist.
3. Program signature refs exist.
4. Usage event refs exist.
5. Usage idempotency refs exist.
6. Exact usage subject refs exist.
7. Attribution refs exist.
8. Pricing policy refs exist.
9. Revenue projection refs exist.
10. Gross revenue is nonzero.
11. Revenue-share split policy refs exist.
12. Fork policy refs exist.
13. License policy refs exist.
14. Dispute policy refs exist.
15. Refund policy refs exist.
16. Payout eligibility refs exist.
17. Contributor payable amount is nonzero and does not exceed gross revenue.
18. Settlement receipt refs exist.
19. Settled contributor amount exactly equals contributor payable amount.

Only after all predicates pass may public copy describe signature/plugin
marketplace revenue as settled or live.

## State Model

The gate projects signature marketplace revenue into these states:

- `blocked`: package validation evidence is missing.
- `validated`: validation exists, but usage evidence is missing.
- `metered`: usage event, idempotency, and exact usage subject refs exist.
- `attributed`: usage has attribution refs.
- `priced`: pricing policy, revenue projection, and gross revenue evidence
  exist. Pending public revenue projection is allowed, but payout and settlement
  claims are blocked.
- `eligible`: revenue-share, fork, license, dispute, and refund policy refs
  exist.
- `payable`: payout eligibility refs and contributor payable amount exist.
  Payout and settlement claims remain blocked.
- `settled`: settlement receipt refs exist.

`settled` is the only state that permits payout claim, settlement claim, or
signature revenue copy.

## Guards

Public signature marketplace refs must reject:

- private package source and private repo/source refs
- raw prompts
- raw usage and raw metering payloads
- provider payloads, grants, accounts, credentials, secrets, and tokens
- customer data and customer email material
- wallet material, payment material, payout targets, invoices, and preimages
- raw timestamps

Package validation and candidate acceptance remain evidence only. Neither grants
install, runtime activation, marketplace listing, payment mutation, payout, or
settlement authority.

## Coverage

Regression coverage lives in:

- `workers/api/src/signature-marketplace-revenue-gate.test.ts`
- `workers/api/src/signature-package-validation.test.ts`

The tests cover:

- validation without install or runtime activation
- exact usage subject and idempotency before metering can advance
- public-safe usage revenue projection without payout before settlement
- settlement receipt refs as the only settled revenue copy path
- fork, license, dispute, refund, and revenue-share split blockers
- impossible revenue-share and settlement amount rejection
- unsafe ref rejection fixtures
- settled projection scan for private material

## Current Gap

OpenAgents product surface now has package validation and a launch gate for signature marketplace
revenue claims. It still does not have a live marketplace that meters package
usage, bills buyers, applies revenue-share splits, handles disputes/refunds, and
settles Bitcoin payouts. Until a settled public-safe usage event has receipt
refs, signature/plugin revenue remains blocked.
