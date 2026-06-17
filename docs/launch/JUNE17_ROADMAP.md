# JUNE 17 ROADMAP — finish wallet unification, publish the RC, close v1.0 gates

Date: 2026-06-17. Carries the open work forward from `JUNE16_ROADMAP.md` (now
closed out). June 15 shipped the launch; June 16 stabilized it and shipped the
Spark wallet unification; this file tracks what's left.

## Where we are (verified 2026-06-17)

- **Launch gate is GREEN and closed (#5012).** Both crucial promises live; the
  Spark offline-receive promise (`payments.offline_receive_spark_fallback.v1`) is
  green on recipient-visible rc.12 evidence.
- **Spark wallet unification (epic #5176) — ALL children closed.** #5177 (Spark
  send/withdraw), #5178 (Spark = primary agent balance), #5179 (chunked treasury
  payouts), #5180 (recipient-attributed + confirmed-receipt ledger), #5181 (MDK
  scoped to checkouts/treasury), #5183 (Spark treasury balance + size-agnostic
  payouts), and **#5182 (recognition closeout — closed 2026-06-17)**. The epic
  itself can close.
- **Treasury holds both MDK + Spark rails** (owner decision: diversification).
  `/api/public/treasury` shows an aggregate balance + rail split, **and now
  surfaces inbound transfers** (the page previously showed outbound only). A
  Spark-treasury BOLT11 funding-invoice endpoint exists (operator-gated) for when
  a funder's wallet can't pay a Spark address.
- **A single 50,000-sat payout settled in one shot** from the Spark treasury to
  Whitefang — first clean 50k in this saga, no sat-size ceiling, no chunking — and
  **Whitefang recipient-confirmed it** (`detectedBalanceSats: 51030`).
- **Pylon rc.13 PUBLISHED** on all three channels: npm `rc` → `1.0.0-rc.13`
  (`latest` stays `0.2.5`), the signed OTA feed (4 platforms, rollout 100, kid
  `2dbe811d`), and GitHub prerelease `pylon-v1.0.0-rc.13`. This is the
  `wallet send` / Spark-primary build the agents were waiting on.
- **Ledger cleaned:** the 3 orphaned pending rows (100,005 sats, all Orrery's
  never-dispatched attempts) are now `expired`; 0 pending outbound remain.

## Open work

### 1. #5182 — launch-recognition closeout — ✅ CLOSED (2026-06-17)

All recognition recipients reconciled by recipient + amount + terminal
recipient-confirmed receipt (#5180), not sender-side "succeeded":

- **Trigger:** 50,000 owed, recipient-confirmed (`detectedBalanceSats: 50000`). Closed.
- **Whitefang:** 50,000 owed; sent in **one** Spark-treasury payment, recipient-confirmed
  (`detectedBalanceSats: 51030` = 1k canary + 50k + 30-sat smokes). Closed.
- **Orrery:** 50,000 owed; over-sent during the overnight debugging, recipient-confirmed
  (`detectedBalanceSats: 159239`). Owner decision: **keep the overage as hazard pay;
  do not resend.** Closed.
- **3 pending rows (100,005 sats):** Orrery's orphaned never-dispatched attempts →
  marked `expired`. 0 pending outbound remain.

### 2. Publish the next Pylon RC — ✅ DONE (rc.13, 2026-06-17)

Published on all three channels: npm `rc` → `1.0.0-rc.13` (`latest` stays `0.2.5`),
the signed OTA feed (`updates.openagents.com`, all 4 platforms, rollout 100, kid
`2dbe811d`, artifacts verified downloadable), and GitHub prerelease
`pylon-v1.0.0-rc.13`. `pylon wallet send` / withdraw and the Spark-primary balance
are now usable by real nodes (npm immediately; binary/Desktop on next auto-update).

### 3. Fix the native Spark transfer path (`invalid_transferid_format`)

The Spark-preferred send path errors with `invalid_transferid_format`, so the
deployed code falls back to `preferSpark:false` (pays the resolved BOLT11 over
Lightning from the Spark wallet). The 50k still settled because the Spark
treasury's Lightning liquidity handled it — but the genuinely native, size-agnostic
`spark1→spark1` transfer is **broken by design right now**, not proven. Fix it so
size-agnostic payouts hold by construction, not by Lightning-liquidity luck — and
so recipients can be paid to a native Spark address, not only a Lightning Address.

### 4. v1.0 release gates (carried from June 16 §H)

- **Gate 1 — Spark fallback payout:** ✅ proven (Trigger + Whitefang 50k landed).
- **Gate 2 — auto-payout dispatch:** ⏳ not wired/tested. Verified work + bounded,
  operator-gated settlement should auto-dispatch the payout (MDK-vs-Spark by
  recipient liveness) and record the receipt, without a human running the command
  per pairing. Approval stays operator-gated/bounded; only dispatch is automated.
- **Gate 3 — auto-update in prod:** ✅ verified end-to-end against the signed feed.
- **v1.0 cut gate:** cut stable v1.0 only after Gate 2 passes and the RC above is
  published, receipt-first with dereferenceable evidence.

### 5. Built-in hosted agent to green (#5063 backend closed)

`autopilot.builtin_compute_agent.v1` is **yellow**. Green needs the desktop
executor calling the live keyless grant route + a from-install go-online smoke + a
signed recut.

### 6. Other active lanes (separate epics, not blocking #5176)

- **#5104** Customer #1 dogfood (run our own work on Forge) + **#5098** onboard 3–5
  teams. **D3.1 (#5200) shipped:** the public-safe cohort evidence ledger now lives
  at `docs/blitz/forge/2026-06-17-customer-one-cohort-evidence-ledger.md`.
  **D3.2 (#5201) shipped:** `/forge` now renders the cohort-readiness lane with
  configured target, absent completion-bundle evidence, absent privacy-review
  evidence, and an explicit awaiting-source gate. **D3.3 (#5203) shipped:** the
  cohort-row source contract now lives at
  `docs/blitz/forge/2026-06-17-customer-one-cohort-source-contract.md`.
  **D3.4 (#5204) shipped:** `workers/api/src/customer-one-cohort-projection.ts`
  now projects private/operator cohort rows into public-safe, evidence-only rows
  with regression coverage in
  `workers/api/src/customer-one-cohort-projection.test.ts`. Remaining D3 tail:
  **D3.5 (#5210) shipped:** `/api/public/customer-one-cohort` now serves the
  typed cohort projection as no-store JSON over an injectable source-store seam.
  **D3.6 (#5212) shipped:** private/operator cohort row intake and storage now
  live behind `/api/operator/customer-one-cohort/rows`, and the public route
  reads that D1 source. **D3.7 (#5215) shipped:** `/forge` now renders cohort
  readiness from `/api/public/customer-one-cohort` with public-safe row labels.
  **D3.8 (#5218) shipped:** `scripts/customer-one-cohort-recorder.mjs` can
  read the public projection and admin-list/upsert real public-safe rows.
  **D3.9 (#5223) shipped:** production now serves
  `/api/public/customer-one-cohort` after the remote D1 migration/deploy, and
  the recorder public smoke reads the projection. Remaining D3 tail: record
  cohort rows and collect at least three real loop-completion bundles before
  closing #5098/#5104.
- **#5107** fold terminal-agent-systems into the Forge Autopilot Coder.
- **Email strategy smoke:** onboard Cloudflare Email Sending, add the restricted
  staging `send_email` binding, send one verified-destination operator
  notification through the `cloudflare_email` adapter.

## Coordination note

Multiple sessions push to `main` in parallel (heavy `.claude/worktrees` + `tmp/oa-*`
worktree activity; this checkout is frequently dirty/behind). Keep work in isolated
worktrees with rebase-before-push; deploy only from a clean `origin/main` checkout,
never the shared working tree. Treasury money-state changes (e.g. expiring the
pending rows, topping up the Spark rail, sending recognition) should be serialized
through one lane to avoid double-spend / double-resolve.
