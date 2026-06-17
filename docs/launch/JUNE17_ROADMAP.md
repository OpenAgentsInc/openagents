# JUNE 17 ROADMAP — finish wallet unification, publish the RC, close v1.0 gates

Date: 2026-06-17. Carries the open work forward from `JUNE16_ROADMAP.md` (now
closed out). June 15 shipped the launch; June 16 stabilized it and shipped the
Spark wallet unification; this file tracks what's left.

## Where we are (verified 2026-06-17)

- **Launch gate is GREEN and closed (#5012).** Both crucial promises live; the
  Spark offline-receive promise (`payments.offline_receive_spark_fallback.v1`) is
  green on recipient-visible rc.12 evidence.
- **Spark wallet unification (epic #5176) is 6/7 done.** Closed: #5177 (Spark
  send/withdraw), #5178 (Spark = primary agent balance), #5179 (chunked treasury
  payouts), #5180 (recipient-attributed + confirmed-receipt ledger), #5181 (MDK
  scoped to checkouts/treasury), #5183 (Spark treasury balance + size-agnostic
  payouts). **Open: #5182** (recognition closeout).
- **Treasury holds both MDK + Spark rails** (owner decision: diversification).
  `/api/public/treasury` shows an aggregate balance with the rail split. A
  Spark-treasury BOLT11 funding-invoice endpoint exists (operator-gated) for when
  a funder's wallet can't pay a Spark address.
- **A single 50,000-sat payout settled in one shot** from the Spark treasury to
  Whitefang — first clean 50k in this saga, demonstrating no sat-size ceiling and
  no chunking required. Treasury then back down to ~1.9k sats.

## Open work

### 1. #5182 — launch-recognition closeout (the only open #5176 child)

Per-recipient state (reconcile by recipient + amount + terminal recipient-confirmed
receipt, per #5180 — not sender-side "succeeded"):

- **Trigger:** 50,000 owed, recipient-confirmed (`detectedBalanceSats: 50000`). **Closed.**
- **Orrery:** 50,000 owed; settled-sent ~159,239–234,639 during the overnight
  debugging over-send; recipient-confirmed `detectedBalanceSats: 159239`. Owner
  decision: **keep the overage as hazard pay; do not resend.** Effectively closed.
- **Whitefang:** 50,000 owed; **sent in one Spark-treasury payment** (`paidVia:
  spark_treasury`, paymentRef `payment.redacted.spark_treasury.c909236d…`, row
  `treasury_payout_a580eac0…`, owed `owed.launch_recognition.whitefang.2026-06-17`,
  ~126-sat fee). `recipient_confirmation_state: unconfirmed` — **awaiting his
  `backup-status` post** to flip to `confirmed_received` and close his line. 1k
  canary already confirmed.
- **3 pending rows (100,005 sats) — ACTIONABLE, not recipient-blocked.** All three
  are Orrery's orphaned attempts (`16:21` 50k tips-buffer BOLT12 that never settled,
  `18:21` 50k, `18:04` 5-sat), pending for 22h+ — impossible for a live Lightning
  payment, so they never dispatched (no funds moved; Orrery is already over-paid via
  settled rows). **Mark them `expired`** to clean the ledger. (Treasury money-state
  change — coordinate with whoever owns the treasury-container lane to avoid a
  collision; do not double-resolve.)

### 2. Publish the next Pylon RC (top unblock for the agents)

#5177/#5178/#5183 landed on `main`, but the npm `rc` dist-tag still resolves to
`1.0.0-rc.12` (Whitefang verified). So `pylon wallet send` / withdraw and
Spark-primary balance are **not yet usable by real nodes**. Cut + publish the next
RC — signed standalone binaries on the OTA feed **and** npm `--tag rc` (keep
`latest` at `0.2.5`) — then confirm the published build in the launch forum thread.
This is what unblocks Trigger and Whitefang actually spending their balances.

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
  teams.
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
