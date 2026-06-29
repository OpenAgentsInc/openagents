# Spark send latency audit (2026-06-17)

Why a Spark `wallet send` takes 7–19 seconds, where the time actually goes, and
what to build to fix it. Measured on real infra, not estimated.

## TL;DR

- A Pylon `wallet send --rail spark` is a **cold, per-command** operation: every
  invocation spins up a fresh process, loads the Spark WASM, builds+connects a new
  SDK session, runs a **full `syncWallet`**, sends, then disconnects.
- **~3.5–4.4 s of every send is pure per-command overhead** (cold start +
  `syncWallet`) that has nothing to do with the payment. It is redone from scratch
  on every command and is **entirely eliminable** with a warm, long-lived Spark
  session in the already-running Pylon daemon.
- The remaining time is the actual Spark→Lightning settle, which is
  **routing-dominated and highly variable** (2.9 s for a 100-sat send, 6.4 s for
  200 sat, ~14 s for a 21,555-sat send). This is not fixed-cost; it is Lightning
  pathfinding + HTLC propagation to the destination LSP.
- Headline fixes: (1) a **warm persistent Spark session** (removes the ~4 s
  overhead from every send), (2) **background sync** (removes `syncWallet` from the
  send path entirely), (3) **Spark-native routing preference** (Spark→Spark
  transfers settle instantly, no Lightning), (4) **optimistic/async send UX** (stop
  blocking the operator on full HTLC settle).

## Method

Instrumented `apps/pylon/src/spark-backup-helper.ts` with opt-in per-step timing
(`PYLON_SPARK_DEBUG=1` → `[spark-timing] <step>=<ms>`, monotonic `performance.now()`,
no payment material). Ran real BOLT11 sends from a funded throwaway Spark wallet on
`pylon-gcp-1` (signed Pylon binary, linux-x64), paying treasury-minted BOLT11
invoices. Wall clock measured externally (`date +%s%3N` around the command).

## Measured breakdown

Two cold sends (each a fresh `pylon wallet send` process):

| Step | 100-sat send | 200-sat send | Nature |
|---|---:|---:|---|
| `process_to_send_closure` (binary start + CLI parse + option resolve) | 656 ms | 638 ms | fixed overhead |
| `module_load` (Spark WASM instantiate) | 218 ms | 203 ms | fixed overhead |
| `sdk_build_connect` (`SdkBuilder…build()`) | 92 ms | 90 ms | fixed overhead |
| `sync_wallet` (`syncWallet({})`) | **3399 ms** | **2474 ms** | fixed overhead |
| `prepare_send_payment` | 139 ms | 258 ms | small |
| `send_payment_settle` (`sendPayment` to completion) | **2886 ms** | **6436 ms** | variable (routing) |
| **wall clock total** | **7582 ms** | **10176 ms** | |

Field data point (Trigger, rc.19, #5196): a 21,555-sat BOLT11 send took **~18.85 s**
end-to-end. Consistent with the model: ~1 s cold start + ~3 s sync + ~14 s settle.

### Reading the numbers

- **Cold start (process + module + build): ~0.95 s, consistent.** Paid on every
  command because each `wallet send` is a brand-new process that loads the WASM and
  builds a fresh SDK.
- **`syncWallet`: ~2.5–3.4 s, consistent.** A full wallet sync on every send. The
  wallet was just built, so it syncs from scratch each time. This is the single
  largest *fixed* cost and is pure overhead relative to the payment.
- **`sdk_build_connect` is cheap (~90 ms)** — the heavy network reconciliation is in
  `syncWallet`, not the builder.
- **`send_payment_settle` is the real payment and is highly variable** (2.9 → 6.4 s
  for 100→200 sat; ~14 s for 21k). It tracks Lightning pathfinding + HTLC settle to
  the destination LSP, not a fixed cost and not strictly amount-linear. We currently
  **block the CLI on full completion** (`completionTimeoutSecs: 60`), so the operator
  waits for the entire settle.

## Root cause

1. **No warm session.** `createSparkBackupSendTransfer` (and the status helper) build
   a short-lived SDK per command — load module → `SdkBuilder.build()` → `syncWallet`
   → send → `disconnect`. The audit's "short-lived sidecar discipline" is safe but
   pays full cold-start + full sync on *every* operation. The Pylon node already runs
   as a long-lived process; the Spark wallet does not live in it.
2. **Full sync on the send path.** Even with a warm session, syncing on every send is
   ~3 s of latency the user waits through. Sync should be a background concern, not a
   precondition of each send.
3. **Synchronous settle.** We wait for the full HTLC settle before returning. For an
   external Lightning destination that is inherently seconds; blocking the operator on
   it is a UX choice, not a requirement.
4. **Lightning when Spark-native would do.** Spark→Spark transfers settle instantly
   (no Lightning routing). Paying another Pylon/treasury Spark wallet over Lightning
   (resolve LA → BOLT11 → route) is the slow path when a native transfer exists.

## Recommendations (route around / abstract / improve)

Ordered by impact-to-effort.

### 1. Warm, persistent Spark session in the Pylon daemon (biggest win)

Keep one Spark SDK session loaded + connected for the life of the Pylon process;
route `wallet send` / `backup-status` / `backup-claim` through it (in-process call or
local IPC) instead of building a cold session per command.

- Removes **cold start (~0.95 s) + build (~0.1 s)** from every command.
- Combined with (2), removes **`syncWallet` (~3 s)** too.
- Net: **~4 s off every send** (the 7.6 s small send → ~3.3 s; the 18.85 s send →
  ~14.5 s) before touching the settle itself.
- Keep the short-lived path as a fallback for one-shot CLI use without a running daemon.

### 2. Background sync (remove `syncWallet` from the send path)

With a warm session, run `syncWallet` on a timer / on relevant events, so the wallet
is already current when a send arrives. The send path then does
`prepare` + `sendPayment` only.

- Removes the **~3 s** sync from the critical path.
- Cheap interim variant even before a full daemon: skip `syncWallet` if a sync
  completed within the last N seconds (requires shared state → effectively needs the
  warm session).

### 3. Spark-native routing preference

Detect when a destination is reachable as a native Spark transfer (another Pylon
agent, the treasury, any Spark address) and use the native rail — it settles
instantly with no Lightning pathfinding. Reserve Lightning (LA → BOLT11 → route) for
genuinely external destinations. `preferSpark: true` is already the first attempt for
BOLT11; extend this to address-level routing and to treasury/agent payouts.

### 4. Optimistic / async send UX

For Lightning sends, return once the payment is **accepted/in-flight** with a
`send-pending`-style receipt, and confirm final settlement in the background (a
follow-up `status` reconciles it). The operator is unblocked in <1 s instead of
waiting 3–14 s for HTLC settle. This composes with the `send-pending` state already
added for #5196 (a pending send is a first-class, safe outcome, not a failure).

### 5. Instrumentation stays

The `[spark-timing]` instrumentation added here ships behind `PYLON_SPARK_DEBUG=1` so
future regressions and per-host differences are measurable, not guessed.

## Proposed sequencing

1. Land a warm Spark session in the daemon + route send/status through it (1+2).
   Expected: cold/overhead drops from ~4 s to ~0.
2. Spark-native routing for agent/treasury destinations (3). Expected: those sends
   drop to sub-second settle.
3. Optimistic async send + reconcile for external Lightning (4). Expected: operator
   unblocked immediately; settle confirmed in background.

After (1)+(2), a typical agent→agent or agent→treasury send should be **sub-second**;
external Lightning sends stay bounded by routing but no longer carry the ~4 s overhead
and no longer block the operator on settle.

## Evidence / repro

- Instrumented binary, `PYLON_SPARK_DEBUG=1`, two cold sends on `pylon-gcp-1`
  (2026-06-17): timings table above.
- Field: Trigger 21,555-sat send ~18.85 s (forum thread `34bebe36`, #5196 confirm).
- Source: `apps/pylon/src/spark-backup-helper.ts` (`createSparkBackupSendTransfer`,
  `buildSparkSdk`, `sendSparkPaymentFromSdk`, `sparkTiming`).
