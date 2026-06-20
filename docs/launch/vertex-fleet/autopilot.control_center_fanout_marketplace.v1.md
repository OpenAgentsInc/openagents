# autopilot.control_center_fanout_marketplace.v1 — marketplace work-class catalog

Date: 2026-06-20
Promise: `autopilot.control_center_fanout_marketplace.v1` (yellow — stays yellow)
Blocker advanced: `blocker.product_promises.plugin_marketplace_beyond_code_task_missing`

## What this adds

A typed **marketplace work-class catalog** — the single registry of the work
classes the Autopilot control-center fanout marketplace can list, and which of
them are actually **live** versus **registered-but-INERT scaffolds**.

Before this, the self-serve fanout (`workers/api/src/self-serve-fanout.ts`)
**hard-coded** a single work class, `code_task`. There was no typed place to
enumerate the additional plugin work classes the marketplace intends to support,
and no single source of truth for which class is live. A reviewer could not tell
from the code whether a class like `data_labeling` was real or aspirational.

`workers/api/src/marketplace-work-class-catalog.ts` provides:

- `MarketplaceWorkClassDefinition` — the typed contract a plugin work class must
  satisfy to be listable: `requiredCapabilityRefs`, `verificationCommandRef`,
  `settlementStream`, and an explicit `status` (`live` | `inert_scaffold`).
- `MARKETPLACE_WORK_CLASS_CATALOG` — the catalog. `code_task` is the **only**
  `live` class (the class #4783 settled under); `data_labeling`,
  `content_writing`, and `research_brief` are registered `inert_scaffold`
  entries — contracts only, nothing wired, no money moves.
- `assertCatalogInvariants` — enforces honesty **in code**: ids are unique,
  `code_task` is present and live, and **no class beyond `code_task` may be
  `live`**. A misedit that flips a plugin class to live throws rather than
  silently over-claiming.
- `isPluginMarketplaceBeyondCodeTaskLive` — the single predicate a future
  green flip must make true; today it is always `false`.
- `projectMarketplaceWorkClassCatalog` — a public-safe projection (yellow,
  inert, `live_at_read`) that reports the still-uncleared plugin blocker.

## Honesty / blocker status

- **Cleared:** none. This change clears no blocker.
- **Stays:** `blocker.product_promises.plugin_marketplace_beyond_code_task_missing`
  — only `code_task` is live; every plugin class beyond it is an inert scaffold.
  The catalog enforces this invariant and reports the blocker on every
  projection. The promise STAYS yellow.

This is the **registry seam** a future owner-armed, receipt-first change would
flip a class to `live` against (per `proof.claim_upgrade_receipts.v1`).

## What genuinely remains for green

1. At least one work class **beyond `code_task`** wired end to end: a provider
   advertising its capability, a validator re-running its verification command,
   and escrow settling on its stream — i.e.
   `isPluginMarketplaceBeyondCodeTaskLive()` becomes `true`.
2. An owner-signed, receipt-first upgrade recording the live plugin class plus a
   settled fanout against an armed self-serve run.

## Files

- `workers/api/src/marketplace-work-class-catalog.ts` — catalog + contract +
  invariants + projection
- `workers/api/src/marketplace-work-class-catalog.test.ts` — 9 tests
  (live/inert split, invariant rejection of an over-claiming live class,
  duplicate/missing-live rejection, honest projection)
- `workers/api/src/marketplace-work-class-catalog-routes.ts` — public read-only
  route `GET /api/public/autopilot/marketplace-work-classes` exposing the catalog
  projection (optional `?workClass=` narrows to one class). No flag/store; honest
  envelope (yellow/inert, plugin blocker uncleared) on every response.
- `workers/api/src/marketplace-work-class-catalog-routes.test.ts` — 4 route tests
  (non-GET 405, honest listing, `?workClass=` narrowing, unknown-id → null)
- Wired into `workers/api/src/index.ts` route table (read-only, no env).

## 2026-06-20 follow-up — catalog made observable

The catalog projection previously had no public surface; a reviewer could only
see it via unit tests. This change exposes it at
`GET /api/public/autopilot/marketplace-work-classes` (read-only, alongside
`/api/public/autopilot/self-serve-fanout`). The blocker still STAYS uncleared —
the route reports `pluginMarketplaceBeyondCodeTaskLive: false` and lists the
plugin classes as `inert_scaffold` on every response.
