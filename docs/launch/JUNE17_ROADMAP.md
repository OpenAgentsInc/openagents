# JUNE 17 ROADMAP - CLOSED OUT (2026-06-18)

> **Status: CLOSED OUT on 2026-06-18.** This is the historical record of the
> June 17 wallet-unification, RC-hardening, launch-gate, and terminal-agent
> systems closeout day. Ongoing June 18 work has moved to
> [`JUNE18_ROADMAP.md`](./JUNE18_ROADMAP.md): launch/test the current Pylon RC
> with the completed Tassadar construction/verification/hybrid logic, prepare
> the Release Candidates forum thread as Raynor, and keep remaining release
> gates receipt-first.

Date: 2026-06-17. Carries the open work forward from `JUNE16_ROADMAP.md` (now
closed out). June 15 shipped the launch; June 16 stabilized it and shipped the
Spark wallet unification; this file tracks what's left.

## END-OF-DAY UPDATE (2026-06-17) — what shipped since the morning (rc.13)

The wallet went from "rc.13 published" to **hardened + proven end-to-end + latency
fixed**, across rc.13 → **rc.22** (all on npm `rc` + signed OTA 4-platform rollout
100 + GitHub prereleases; `latest` stays `0.2.5`):

- **Spending PROVEN with real sats, receipt-first:** BOLT11 send (#5185 — root
  cause was a non-UUID idempotency key → `Invalid TransferId format`; fixed via the
  treasury sender's UUID TransferId + `preferSpark` fallback), large/slow sends
  (#5196 — the 15 s send timeout aborted slow sends that then completed; fixed by
  waiting the full completion window + a `send-pending` indeterminate state — no
  blind-retry, no double-pay), and Lightning-Address sends (#5208 — the SDK's
  `lnurlPay` "Tree service error: insufficient funds"; fixed by resolving LA→BOLT11
  and paying via the proven `sendPayment` path). **#5185/#5196 closed
  (contributor-confirmed); #5208 shipped, awaiting Whitefang's owner-approved
  retest.**
- **Read path hardened:** #5184 (version-drift loop + `wallet status` hang) closed;
  #5197 (stale post-restart balance read → 45 s force-sync window + a
  `balanceRefreshing` flag) **closed, Trigger-confirmed**; #5194 (read
  `helper-unavailable` on one macOS host → fallback + diagnostic) shipped, awaiting
  Orrery's retest.
- **Epics/closeouts:** **#5176 (Spark wallet unification) CLOSED**; **#5078 (the
  original offline-receive gap) CLOSED** (Trigger + Whitefang recipient-confirmed);
  #5182 recognition closeout CLOSED. Product-promise registry updated to
  `2026-06-17.3`.
- **Per-send latency: overhead eliminated (#5207 — CLOSED, verified).** A warm,
  daemon-resident Spark session + background sync removes the ~4 s cold build +
  `syncWallet` from every send (measured: a daemon-routed send skipped the sync and
  did no rebuild). Audit: `apps/pylon/docs/2026-06-17-spark-send-latency-audit.md`.
- **In progress (delegated, under review):** **#5225** Spark-native routing
  (Spark→Spark settles **0-fee, ~3.6 s, no Lightning-routing variance** — measured;
  making it explicit + preferring Spark addresses for internal flows) and **#5232**
  real settlement / Gate 2 (flip accepted-work/training payouts from simulation to
  real Bitcoin — gated scaffold being designed; real money stays owner-gated).
- **Strategy:** Ark/bark adoption audit written
  (`docs/2026-06-17-ark-bark-adoption-audit.md`) — recommends a signet PoC of Ark
  as a third rail behind the wallet abstraction.
- **Forge Autopilot Coder #5107 CLOSED.** The terminal-agent-systems
  operationalization wave is now folded into the Forge `/autopilot` Run-detail
  surface through the current #5107 scope. The work spans the original G1-G7
  evidence foundation, the H1-H6 operational follow-on, and the I1-I43 system
  lanes, with the parent issue closed after the final audit found no remaining
  open child issue referencing #5107. Everything is intentionally refs-only:
  the browser can inspect typed evidence, blockers, receipts, and public-safe
  summaries, but it does not gain runtime execution, provider-call, workspace
  write, approval-grant, public-claim, accepted-outcome, payout, or settlement
  authority.
- **What changed in the product surface for #5107:** `/autopilot` now has
  first-class Run-detail lanes for review/change evidence, plan/progress,
  session navigation and control receipts, context/repository memory,
  retrieval/search planning, Help/doctor/debug, MCP/skill/hook/plugin/settings
  readiness, guarded extensibility execution requests, error recovery,
  compaction, usage and budget, model provider resolution, instruction
  layering, session/team memory, notifications, command/input/keybinding,
  browser/desktop/editor/git/external-work integration, MCP server export,
  scheduling, artifact receipts, migration, structured event logs,
  credential storage, telemetry/privacy, performance diagnostics, multi-agent
  coordination, companion surface, update/release, testing/evaluation/security/
  retention/onboarding evidence, output style/persona, prompt suggestions,
  tips/education, multimodal input, terminal UI shell, theme/visual design,
  accessibility and non-interactive mode, localization boundaries, and
  enterprise managed policy.
- **Final #5107 verification:** the last focused lane (#5281 / I43 Enterprise
  managed policy) passed `2 files / 126 tests`; the full terminal-agent-systems
  web bucket passed `63 files / 633 tests`; and `git diff --check` passed. Full
  `bun run --cwd apps/openagents.com/apps/web typecheck` is still blocked by
  unrelated baseline scene and SpacetimeDB binding errors (`spacetimedb`,
  `@openagentsinc/three-effect` WASD look exports, `tassadarRunElement`,
  `tassadarRunSnapshot*`, and `tassadarSpacetimeWorld` generated-binding
  mismatches), not by the #5107 evidence lanes.

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
- **Forge Autopilot Coder terminal-agent systems (#5107) is closed for this
  wave.** The launch posture is now implementation-complete for the current
  terminal-agent-systems incorporation plan, with public-safe refs-only
  projections in the Run detail surface and documented verification in
  `docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`.
  Broader product-promise/copy claims still need the separate G7 signed-deploy
  and live-smoke evidence gate.

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

### 3. Native Spark transfer — ✅ send fixed (#5185); native routing in progress (#5225)

The `Invalid TransferId format` error is **fixed** (#5185 — it was a non-UUID
idempotency key, not the amount; the treasury sender's UUID TransferId resolved
it), and `wallet send` is proven for BOLT11 / large / Lightning-Address
destinations. The genuinely native `spark1→spark1` transfer is now confirmed
working and **measured: 0-fee, ~3.6 s, no Lightning-routing variance** — tracked as
**#5225** to make native routing explicit (classify a Spark-address destination →
native, never fall back to Lightning, label `method: spark_native`) and to **prefer
Spark addresses for internal flows** (agent↔agent, agent↔treasury) so those payouts
go native by construction instead of over Lightning. In progress (delegated, under
review). Owner directive: native whenever possible, Lightning as fallback.

### 4. v1.0 release gates (carried from June 16 §H)

- **Gate 1 — Spark fallback payout:** ✅ proven (Trigger + Whitefang 50k landed).
- **Gate 2 — real settlement / auto-payout dispatch:** ⏳ in progress as **#5232**
  (the biggest milestone left). Flip accepted-work/training settlement from
  simulation (`movementMode:'simulation'` / `realBitcoinMoved:false`) to REAL
  Bitcoin actually moving over the now-proven Spark rail — idempotent, bounded,
  owner-gated — with a `realBitcoinMoved:true` receipt and the receipt-first
  promise flip. A gated scaffold (defaults to simulation; real money only behind an
  explicit owner-gate + sat cap) is being designed; real-money enablement +
  verification stay owner-gated and with the lead, not auto-shipped.
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
  the recorder public smoke reads the projection.
  **D3.10 (#5226) shipped:** the cohort recorder now has a local `check`
  command, and the source contract links a public-safe row packet template.
  **D3.11 (#5230) shipped:** the cohort recorder now has a public `audit`
  command that fails until the projection gate is ready and at least three
  counted completions exist. **D3.12 (#5233) shipped:** the cohort source
  contract now links a privacy-review checklist required before a completion
  row can carry `privacyReviewRef`. **D3.13 (#5241) shipped:** production now
  records the first internal Customer #1 completion row, and the public
  projection reports 1/3 counted completions. **D3.14 (#5243) shipped:**
  production now records the second internal Customer #1 completion row, and
  the public projection reports 2/3 counted completions.
  **D3.15 (#5244) shipped:** production now records the third internal
  Customer #1 completion row, the projection gate is ready with 3/3 counted
  completions, and the public audit passes. D3 is complete, #5098/#5104 are
  closed, and the closeout audit lives at
  `docs/launch/2026-06-17-customer-one-dogfood-audit.md`.
- **#5107 fold terminal-agent-systems into the Forge Autopilot Coder — CLOSED
  (2026-06-17).** The implementation record now lives in
  `docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`.
  The completed scope includes #5123-#5144 (first refs-only projection and
  readiness foundation), #5145-#5150 (public-safe drilldowns and guarded
  operational receipts), and #5198-#5281 (the I1-I43 terminal-agent system
  lanes). Final proof: 63 web test files / 633 tests passed for the full
  terminal-agent-systems bucket, `git diff --check` passed, and the parent
  issue was closed after the no-open-child audit.
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
