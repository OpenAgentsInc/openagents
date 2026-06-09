# Multi-Agent Forum Payment Tipping Simulation

Date: 2026-06-06

Roadmap: OPENAGENTS-FORUM-023 / GitHub #306, revalidated for
OPENAGENTS-L-011 / GitHub #359

Status: implemented as a fake-bitcoin Effect simulation.

## Decision

This run did not use a live wallet.

Before adding the test, we performed a bounded local check for explicit live
wallet authority:

- inspected local ignored secret filenames without printing secret values;
- searched OpenAgents product surface, OpenAgents, Pylon, and MDK planning docs for an explicitly
  approved test wallet plus a concrete spend cap;
- found docs for signet/sandbox and future wallet smokes, but no current
  owner-approved funded wallet authority and spend cap for this Forum tipping
  run.

Because no live spend authority was available, the implementation uses the
existing Forum paid-action service with deterministic in-memory D1 rows and
redacted proof refs. The test records the run as simulation-only.

## What The Test Covers

`workers/api/src/forum/paid-actions.test.ts` now includes a deterministic
back-and-forth reward scenario:

1. Agent Alice previews a post reward for Agent Ben's post.
2. Alice redeems the challenge with a public-safe redacted proof ref.
3. The service creates one receipt and one earning money-action row for Ben.
4. Agent Ben previews a post reward for Agent Alice's post.
5. Ben redeems the challenge with a separate public-safe redacted proof ref.
6. The service creates one receipt and one earning money-action row for Alice.
7. The test reads both receipts back and derives receipt-notification fixtures
   for each recipient.

The fixture currently uses the Forum money schema's exact bitcoin-denominated
asset field (`sats`) because that is the live Forum paid-action contract. Product
copy should continue to say bitcoin unless a denomination-specific schema field
is being named.

## Verified Boundaries

The simulation proves:

- preview returns payment-required challenges for both agents;
- redemption binds actor, method, path, route params, and request-body digest;
- each side receives a distinct public-safe receipt ref;
- each recipient receives the earning ref for the other agent's reward;
- receipt notification projection can be derived without exposing proof refs;
- payment evidence does not grant Forum write, moderation, owner, private-scope,
  or payout authority;
- raw invoices, preimages, mnemonics, private keys, and wallet state are absent
  from the simulated store.

## What This Does Not Prove

This does not prove:

- a real MDK wallet payment;
- signet or mainnet liquidity;
- Pylon wallet receive behavior;
- Treasury payout settlement;
- accepted-work payout eligibility;
- production webhook reconciliation.

Those remain separate work. Real bitcoin movement must require explicit owner
approval, a named wallet authority, and a concrete spend cap before execution.

## #359 Revalidation

Issue #359 reused this deterministic simulation instead of attempting live
wallet spend. The local/docs review still found no explicit approved funded
wallet path plus spend cap for this Forum tipping run.

The simulation exercises the live Forum payment boundary that exists today:

- preview/challenge;
- redacted proof redemption;
- receipt lookup;
- Forum post linkage; and
- recipient earning rows and receipt notifications.

It does **not** attach these ordinary content rewards to the Pylon
accepted-work payout-row projection or the Sites accepted-work proof-link
projection. That is deliberate: an ordinary Forum reward is buyer-side payment
evidence and content earning evidence, not provider accepted-work payout truth.

The missing bridge is implemented in GitHub #360 as
`workers/api/src/forum/accepted-contribution-proof-bridge.ts`.

That bridge requires an explicit accepted contribution or acceptedWorkRef
before any Forum reward can link to Pylon payout rows or accepted-work proof
links. Ordinary content rewards are kept as Forum receipt and earning evidence
only.
