# R10 Artanis/Pylon Campaign Ledger

Date: 2026-06-06

Status: implemented contract note for GitHub issue #320 / `OPENAGENTS-073`.

## Purpose

The R10 campaign should make Artanis useful as a public Pylon campaign agent
without overclaiming what Pylon, provider routing, Bitcoin accounting, or
settlement can currently prove.

The implementation lives in `workers/api/src/r10-pylon-campaign.ts`.

## Contract

`R10PylonCampaignInput` records the campaign, the public Artanis agent ref,
source refs, and claim entries.

Each `R10PylonCampaignClaimEntry` records:

- campaign area;
- claim kind;
- desired public claim state;
- evidence refs;
- caveat refs;
- next-action refs;
- participant-action refs;
- blocker refs;
- updated time.

Projection uses the existing public claim-state and copy-rule system. The
projected state may be lower than the desired state when evidence is missing.
For example, a desired `settled` claim without settlement evidence projects as
`verified`, not `settled`.

## Current Campaign State

The seeded ledger currently projects:

- measured public Artanis/Pylon surface;
- verified Pylon setup instruction packet;
- planned next Pylon release artifact;
- planned bounded Pylon work-routing slice;
- modeled accepted-work Bitcoin accounting;
- blocked live-wallet Forum tipping smoke until a named wallet authority and
  spend cap exist;
- prohibited settled-provider-payout claim until a public settlement receipt
  chain exists.

This keeps the public campaign useful while making the missing proof explicit.

## Agent Guidance

Agents and humans may inspect:

- `https://openagents.com/agents/artanis`;
- `https://openagents.com/api/public/pylon-stats`;
- the public Forum tipping simulation note;
- the Pylon setup packet;
- public claim-state/caveat refs.

They may propose next steps, but they must not:

- install or run Pylon without owner approval;
- spend bitcoin or redeem L402 challenges without explicit wallet authority and
  a spend cap;
- claim live wallet testing when only fake-bitcoin simulation exists;
- claim provider payout settlement without settlement evidence;
- expose wallet, payment, provider, runner, or private customer material.

## Verification

`workers/api/src/r10-pylon-campaign.test.ts` covers:

- measured, verified, planned, modeled, blocked, and prohibited claim states;
- live-wallet tipping blocked state;
- prohibited provider payout settlement state;
- settlement-claim lowering when settlement evidence is absent;
- raw timestamp omission;
- redaction rejection for wallet, provider, runner, customer, payment, and raw
  timestamp material.
