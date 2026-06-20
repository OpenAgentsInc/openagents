# pylon.consumer_compute_earns_bitcoin_self_serve.v1 — vertex-fleet note

Date: 2026-06-20
State: red (UNCHANGED — no promise flip in this change)

## Update 2026-06-20 (d) — scale methodology: cross-contributor settlement integrity

Blocker advanced this run:
`blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`

The conformance verifier already rejected inflated counts, simulation/non-settled
receipts, and duplicate `pylonRef`s. But it checked only that *contributors* were
distinct — not that each counted contributor was backed by its OWN distinct real
settlement. Two contributors with different `pylonRef`s could both cite the SAME
provider-confirmed real-bitcoin receipt and still conform, falsely backing "two
distinct real-paid contributors" with a single Bitcoin movement. The promise
claim explicitly rests on *two distinct settlements* (1,005 sats), so this was a
real integrity gap.

- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.ts` —
  added `QualifiedRunReason.SharedSettlementReceipt` and a run-level check that
  flattens the counted contributors' `countedSettlementReceiptRefs` and fails
  conformance when any receipt is reused across counted contributors.
- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.test.ts`
  — +2 vitest cases (shared-receipt fails; own-distinct-receipt conforms); now
  15 tests, wired into `check:deploy`.
- Methodology doc updated to document the cross-contributor integrity rule.

No promise state changed; no scale claim asserted. Still listed: clearing it
needs running the verifier against the live run's real evidence and citing
`conforms:true`, plus owner sign-off.

## Update 2026-06-20 (c) — applied README copy-drift guard (real file, not fixture)

Blocker advanced this run:
`blocker.product_promises.windows_wsl_consumer_install_coverage_missing`

The prior run made the copy-narrowing decision machine-checkable via
`verifyConsumerInstallPlatformClaim`, but that verifier only ran against
SYNTHETIC claim objects — it could not catch a future edit to the actual shipped
consumer-facing copy. This run binds the verifier to the real file:

- `apps/pylon/src/consumer-install-platform-support.ts` — added
  `auditReadmePlatformCopy(readmeText)` (pure): derives a claim from the README
  text (supported set stays `{darwin, linux}`; over-promise phrases flip the
  matching scope flag), runs the existing verifier, and returns `copyHonest`.
  Plus `README_NARROWED_PLATFORM_SENTENCE` (source-of-truth narrowing sentence)
  and `OVERPROMISE_COPY_PATTERNS` (public-safe any-platform / windows / wsl
  detectors).
- `apps/pylon/tests/consumer-install-readme-copy-guard.test.ts` — 6 bun:test
  cases that read the real `apps/pylon/README.md`, assert the shipped copy is
  honest today, and prove the guard fails on drift (reintroduced any-platform /
  Windows claim, or removed narrowing sentence).
- `apps/pylon/docs/platform-support.md` — documented the applied guard.

No promise state changed; no Windows/WSL support claimed; no host probed. Still
listed: clearing it needs the owner-facing copy-narrowing sign-off. This run
makes that decision enforceable against the real shipped file, not optional.

## Update 2026-06-20 (b) — Windows/WSL platform-claim drift guard

Blocker advanced this run:
`blocker.product_promises.windows_wsl_consumer_install_coverage_missing`

This blocker previously had only prose (`apps/pylon/docs/platform-support.md`)
and no code. Per the promise verification text, the honest path is NOT to build
Windows support — it is to keep the public copy narrowed to the proven platforms
(macOS/Linux) and stop it drifting back to "anybody on any platform" / "Windows
covered". This run made that requirement machine-checkable:

- `apps/pylon/src/consumer-install-platform-support.ts` —
  `classifyConsumerInstallPlatform` (pure, public-safe per-platform disposition;
  `supported` for darwin/linux via the shared `bootstrap.isSupportedPlatform`,
  `out-of-scope` for `win32`/WSL/other with honest guidance + blocker ref) and
  `verifyConsumerInstallPlatformClaim` (audits an untrusted stated claim, flags
  `overpromises` when the supported set isn't exactly `{darwin, linux}` or names
  windows/wsl/any-platform; closed key allowlist).
- `apps/pylon/src/consumer-install-platform-support.test.ts` — 14 bun:test cases
  (pass).
- `apps/pylon/docs/platform-support.md` — added a "Copy-Drift Guard" section
  dereferencing the verifier.

No promise state changed; no Windows/WSL support claimed; no host probed. Still
listed: clearing it needs the owner-facing copy-narrowing sign-off (the guard
now makes that decision enforceable, not optional).

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
