# Customer #1 Spend Routing

Date: 2026-06-16
Scope: #5096, Epic D / customer #1 dogfood.

## What Shipped

Internal Forge work-order list recovery now exposes a compact routing summary
for each work order returned by:

`GET /api/autopilot/work?promiseId=...`

The summary is derived from the existing Autopilot placement projection rather
than a parallel selector. It includes:

- whether placement selected an owned requester Pylon, a fallback lane, or no
  compatible runner;
- the selected runner kind and fallback runner kind refs;
- whether the active lane requires buyer debit;
- the active lane ref and meter kind;
- fallback lease intent and Pylon assignment intent counts.

The `/forge` customer #1 factory strip now renders a spend-routing row from that
list projection:

- owned-node work;
- fallback-lane work;
- metered work;
- blocked routing.

This makes the dogfood loop visible enough to see whether OpenAgents internal
AI/coding work is actually flowing through owned Pylons or the fallback account
pool.

## Authority Boundary

The routing summary is operator-safe projection data only. It does not grant
runtime authority, provider-account mutation, spend authority, payout authority,
settlement authority, accepted-work authority, or merge authority.

`buyerDebitRequired` reports the placement lane requirement. It is not a durable
ledger entry and does not prove that a debit, payment, payout, or settlement
happened. Durable spend and payment evidence remain separate receipt-backed
records.

## Public Safety

The list projection exposes only refs, enum-like runner/lane kinds, booleans,
and counts. It does not expose raw prompts, raw shell logs, private repository
content, provider payloads, wallet material, payment secrets, local paths, or
customer-private data.

## Verification

Regression coverage lives in:

- `apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts`, which
  asserts the list endpoint returns owned-Pylon and metered fallback routing
  summaries.
- `apps/openagents.com/apps/web/src/page/loggedIn/view.scene.test.ts`, which
  asserts `/forge` renders owned-node and fallback-lane routing counters from
  the loaded work-order list.
