# Artanis Standalone Autonomy Claim Ledger

Date: 2026-06-06

Status: implemented in #396 / `ARTANIS-011`.

## Purpose

Artanis needs one public claim ledger for standalone autonomy, not only the R10
Pylon campaign ledger.

The ledger tells `/artanis`, Forum copy, launch docs, and operator review what
can be said publicly about Artanis without overstating the state of the actual
runtime.

## Claim Areas

The implemented areas are:

- autonomous loop;
- operator steering;
- Forum communication;
- Pylon campaign;
- Nexus/Pylon administration;
- Model Lab stewardship;
- work routing;
- spend authority;
- bitcoin rewards;
- accepted-work payout;
- settlement.

## Claim States

The ledger uses the existing `PublicClaimState` vocabulary:

- `planned`
- `modeled`
- `measured`
- `verified`
- `blocked`
- `prohibited`
- `settled`

There is not a separate public `paid` state in OpenAgents product surface yet. Payment-related
claims are represented as `blocked`, `prohibited`, `measured`, `verified`, or
`settled` under the existing public claim contract.

That is intentional. Public copy can say that a bitcoin reward or accepted-work
payout path is planned, modeled, measured, blocked, or settled only when the
corresponding evidence exists. It must not invent an in-between payment state
that could be confused with final settlement.

## Current Seeded Truth Table

The current example ledger projects:

- measured autonomous loop projection, because `/artanis` and the loop contract
  exist as public-safe records;
- verified operator steering and approval-gate contracts;
- verified Forum communication contracts and Artanis Forum taxonomy;
- measured R10 Pylon campaign state;
- planned Nexus/Pylon administration adapters;
- verified Model Lab stewardship through public report contracts;
- modeled work routing, because proposals exist but dispatch authority does not;
- blocked spend authority until an operator-approved spend gate exists;
- blocked bitcoin rewards until the Forum reward smoke exists;
- prohibited accepted-work payout claims until public accepted-work receipt
  chains exist;
- prohibited settlement claims until public settlement receipt chains exist.

## Public Projection

Code lives in:

- `workers/api/src/artanis-standalone-claim-ledger.ts`
- `workers/api/src/artanis-public-report.ts`
- `apps/web/src/page/loggedOut/page/publicAgent.ts`

The public Artanis report now includes both:

- `standaloneClaims`: the full standalone autonomy ledger;
- `r10Claims`: the narrower R10 Pylon campaign ledger.

The `/artanis` page renders standalone autonomy claims before R10 campaign
claims so public copy starts with the broad Artanis state and then drills into
the current Pylon campaign.

## Safety Boundary

The ledger rejects:

- non-`agent_artanis` identity;
- missing required claim areas;
- provider, runner, wallet, payment-secret, customer, email, private repo,
  secret, raw prompt, raw log, raw payload, raw source archive, and raw
  timestamp material;
- unsafe Forum copy refs.

Evidence-sensitive claims are lowered by the shared claim-state contract when
required evidence is missing. For example, a desired `verified` claim without
evidence projects as `planned` with a public caveat explaining that the claim
was lowered.

## Verification

Coverage lives in:

- `workers/api/src/artanis-standalone-claim-ledger.test.ts`
- `workers/api/src/artanis-public-report.test.ts`
- `apps/web/src/docs-blog-route.test.ts`

The tests cover:

- every required standalone autonomy claim area;
- measured, verified, planned, modeled, blocked, and prohibited projections;
- honest bitcoin reward, accepted-work payout, and settlement claims;
- lowering of false verified claims when evidence is missing;
- rejection of non-Artanis identity, missing required areas, unsafe Forum copy
  refs, raw timestamps, provider material, runner material, wallet material,
  payment material, and customer material;
- `/artanis` using `standaloneClaims` alongside `r10Claims`.
