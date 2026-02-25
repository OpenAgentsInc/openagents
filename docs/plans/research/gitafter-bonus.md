# Optional Plan: Agent Git Platform (GitAfter Bonus)

Status: Optional (bonus surface)
Date: 2026-02-23
Owner: OpenAgents

This document holds the Agent Git Platform work that was intentionally carved out of
`docs/plans/2026-02-23-open-agent-economy-execution-plan.md` so the primary plan can stay
liquidity-first (Autopilot + OpenAgents Compute).

## Purpose

Provide an optional, Nostr-native Git collaboration surface with bounties, claims, patch proofs,
and merge-triggered payouts.

This is not required to bootstrap compute marketplace liquidity.

## Dependencies (Non-Exhaustive)

- Nostr protocol and required NIPs are available (see `OA-ECON-025` to `OA-ECON-039`).
- Wallet, payments, and receipt canonicalization are available (see `OA-ECON-100`+, `ADR-0005`).
- Canonical replay artifacts exist for verification and dispute evidence (`OA-ECON-179`).

## Sequencing (Timeline-Free)

1. Emit and ingest repo events (baseline primitives).
2. Add bounty/claim flows.
3. Bind patches to trajectory proofs.
4. Add merge-triggered settlement and validation.
5. Run dedicated integration tests for payout correctness.

## Issue Catalog (Moved Out of Main Plan)

- `OA-ECON-105` - Implement GitAfter repository events on NIP-34. - Represent repos and collaboration primitives as protocol events.
- `OA-ECON-106` - Implement bounty issue events and claim flows. - Attach economic incentives to decentralized issue lifecycle.
- `OA-ECON-107` - Implement patch events with trajectory proofs. - Bind code changes to verifiable reasoning artifacts.
- `OA-ECON-108` - Implement stacked diff dependency graph. - Support layered patch sequencing and merge order guarantees.
- `OA-ECON-109` - Implement merge-triggered bounty settlement. - Release payouts automatically on verified merge events.
- `OA-ECON-114` - Build GitAfter and payment integration suite. - Validate bounty and trajectory payout correctness.

## Release Gates (Optional Surface)

- Gate GA: Repo -> bounty -> claim -> patch -> merge -> payout works end-to-end with deterministic receipts and replay artifacts.
- Gate GB: Duplicate/timeout paths are idempotent; no double payouts under retries.
- Gate GC: Dispute evidence bundles are sufficient to reconstruct and adjudicate payout correctness.
