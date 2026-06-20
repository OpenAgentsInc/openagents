# pylon.consumer_compute_earns_bitcoin_self_serve.v1 — vertex-fleet note

Date: 2026-06-20
State: red (UNCHANGED — no promise flip in this change)

## Blocker advanced

`blocker.product_promises.spark_helper_autostart_receipt_missing`

## What was built

A deterministic, public-safe **receipt verifier** for the Spark-helper autostart
receipt — the missing audit gate that a captured receipt must pass before it
could ever be cited to clear the blocker.

- `apps/pylon/src/spark-helper-autostart.ts` — added
  `verifySparkHelperAutostartReceipt(candidate)` returning
  `{ valid, clearsBlocker, reasons[] }`. Pure / side-effect-free. Enforces a
  closed key allowlist (rejects any leak-prone extra field), correct
  schema/ref/types, ref↔body state agreement, payout-ready state, no operator
  hand-start, redaction, and a canonical ISO-8601 timestamp.
- `apps/pylon/src/spark-helper-autostart.test.ts` — +9 verifier tests
  (18 pass total).
- `apps/pylon/docs/spark-helper-autostart-receipt-capture.md` — capture &
  verification runbook for the normal-contributor self-serve path.

## What this deliberately does NOT do

- No promise state changed; green count untouched.
- The autostart capability remains INERT (default off, `PYLON_SPARK_AUTOSTART`).
- No real receipt captured, no helper started, no funds moved, no wallet touched.
- No secrets/targets/balances emitted; the verifier itself rejects such fields.

## What genuinely remains for this promise (still red)

- `spark_helper_autostart_receipt_missing`: a REAL captured autostart receipt
  for ≥1 normal contributor that passes the verifier with `clearsBlocker:true`.
- `windows_wsl_consumer_install_coverage_missing`: narrow the broad
  "anybody on any platform" copy to macOS/Linux (owner scope-out).
- `consumer_compute_self_serve_scale_methodology_missing`: apply the existing
  scale methodology to an actual run.
- Owner sign-off, receipt-first per `proof.claim_upgrade_receipts.v1`.
