# pylon.consumer_compute_earns_bitcoin_self_serve.v1 — vertex-fleet note

Date: 2026-06-20
State: red (UNCHANGED — no promise flip in this change)

## Update 2026-06-20 — scale-methodology conformance verifier

Blocker advanced this run:
`blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`

The participant/scale methodology was already written
(`docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md`)
but the promise verification requires the methodology be *applied* as an
enforceable gate. This run built that gate:

- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.ts` —
  pure `verifyQualifiedContributorMethodology` / `verifyQualifiedContributor`.
  Recomputes a run's qualified-contributor count from per-contributor evidence
  under the authoritative 3-prong rule (admitted lease + replay-verified
  exact_trace work + provider-confirmed real-bitcoin settlement) and flags an
  inflated/under-counted claim, double-counts, and excluded receipts. Explicitly
  rejects simulation-only (`realBitcoinMoved:false`), non-`settled`, and
  not-provider-confirmed receipts — closing the gap where the in-line
  `qualifiedContributorRefs` join trusts its caller to pre-filter receipts.
- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.test.ts`
  — 13 vitest cases, wired into `apps/openagents.com` `check:deploy`.
- Methodology doc updated to dereference the verifier.

No promise state changed; no scale claim asserted. Still listed; the honest
remaining step is running the verifier against the live run's real evidence and
citing `conforms:true`, plus owner sign-off. The other two blockers below remain.

## Prior run — Spark-helper autostart receipt verifier

Blocker advanced (prior run):
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
