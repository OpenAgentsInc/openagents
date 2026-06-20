# Spark-helper autostart receipt — capture & verification

Promise: `pylon.consumer_compute_earns_bitcoin_self_serve.v1`
Blocker:  `blocker.product_promises.spark_helper_autostart_receipt_missing`

The video-core promise ("anybody plugs in consumer compute and gets paid
Bitcoin") is partly blocked because there is **no captured receipt** proving a
NORMAL contributor's Spark backup helper reaches payout-readiness on the
self-serve path *without an operator hand-starting it*.

`apps/pylon/src/spark-helper-autostart.ts` supplies the inert, flag-gated
capability (classifier + receipt builder). This doc adds the auditable bar a
captured receipt must clear before it could ever be cited against the blocker,
implemented as `verifySparkHelperAutostartReceipt`.

## What is now buildable / verifiable

- `classifySparkHelperAutostart(receive, opts)` — decides autostart readiness
  from an already-computed `SparkBackupReceiveProjection`. **INERT by default**:
  returns `disabled` unless `PYLON_SPARK_AUTOSTART=1` (or `{ enabled: true }`).
- `buildSparkHelperAutostartReceipt(projection, observedAt)` — emits a redacted,
  public-safe receipt **only** when the projection is `autostart-ready`.
- `verifySparkHelperAutostartReceipt(candidate)` — **new**: deterministic audit
  over an untrusted candidate (e.g. a JSON artifact a contributor captured).
  Returns `{ valid, clearsBlocker, reasons[] }`.

## Capture procedure (normal contributor, self-serve)

1. Install Pylon and complete Spark backup receive setup (see
   `2026-06-15-spark-backup-receive-runbook.md`).
2. Opt in: set `PYLON_SPARK_AUTOSTART=1`. With the flag off nothing changes.
3. Without any operator hand-start, let the helper reach a payout-ready receive
   target (`address-ready` or offline `cached-address-ready`).
4. Capture: classify the receive projection, then build the receipt with the
   observation timestamp, and write the JSON artifact. Do **not** hand-edit it.

## Verification gate (auditor / reviewer)

Run `verifySparkHelperAutostartReceipt(candidate)` against the captured JSON.
A receipt may be cited against the blocker **only when** `clearsBlocker === true`.
The audit fails (and the artifact must be rejected) on any of:

- `not-an-object` — candidate is not a plain object.
- `unexpected-key:<k>` — a key outside the closed allowlist is present. This is
  the public-safety guard: a leaked balance / raw address / credential field can
  never ride along inside a "valid" receipt.
- `bad-schema` / `bad-ref` — wrong schema string or ref shape.
- `ref-state-not-payout-ready` / `derived-state-not-payout-ready` — the encoded
  receive state is not a payout-ready target.
- `ref-state-mismatch` — the ref's encoded state disagrees with the body's
  `derivedFromReceiveState`.
- `not-payout-ready` — `payoutReady !== true`.
- `operator-hand-start-required` — `operatorHandStartRequired !== false` (the
  core anti-claim: a receipt requiring an operator does NOT prove self-serve).
- `not-redacted` — `contentRedacted !== true`.
- `bad-observed-at` — `observedAt` is not a canonical ISO-8601 timestamp.

## What this does NOT do (honesty)

- It does **not** capture a real receipt, start a helper, spawn a process, move
  funds, or touch the wallet. The flag stays off; live behavior is unchanged.
- It does **not** clear `spark_helper_autostart_receipt_missing`. Green still
  requires a REAL captured receipt for ≥1 normal contributor that passes
  `verifySparkHelperAutostartReceipt` with `clearsBlocker === true`, plus the
  separate copy-narrowing and scale-methodology work, plus owner sign-off
  (receipt-first per `proof.claim_upgrade_receipts.v1`).

Tests: `apps/pylon/src/spark-helper-autostart.test.ts` (18 pass).
