# pylon.consumer_compute_earns_bitcoin_self_serve.v1 — vertex-fleet note

Date: 2026-06-20
State: red (UNCHANGED — no promise flip in this change)

## Update 2026-06-20 (o) — scale methodology: prove the real file→parse→verify path with a canonical evidence template

Blocker advanced this run:
`blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`

The verifier, parse boundary, and fused `verifyQualifiedContributorMethodologyDocument`
entry are all built, but the documented remaining step — "run the verifier
against the live run's REAL evidence **file**" — was never exercised end to end:
every existing test builds the document in-memory (even the "JSON round-trip"
case stringifies an in-memory object). There was also no checked-in TEMPLATE an
auditor could copy to assemble the real evidence file, so the exact on-disk shape
the verifier expects was only implicit in code.

- `apps/openagents.com/workers/api/src/fixtures/qualified-contributor-methodology-evidence.template.json`
  — NEW canonical, public-safe SHAPE TEMPLATE for the per-run evidence document
  (two distinct contributors, each with distinct lease / verified-work /
  provider-confirmed settlement refs). Strictly schema-conformant so it passes the
  closed key allowlist; every ref is a self-evident placeholder
  (`pylon.example.*`, `lease.example.*`, `receipt.example.*`). It is synthetic and
  asserts no real claim.
- `apps/openagents.com/workers/api/src/fixtures/qualified-contributor-methodology-evidence.template.README.md`
  — NEW note documenting that the template is synthetic, why annotation keys can't
  live in the JSON (the allowlist would reject them), and exactly what still
  remains to clear the blocker.
- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.test.ts`
  — +4 vitest cases in a new "evidence document template" suite that READ the
  template from disk (`readFileSync` + `import.meta.url`) and run the real
  file→`JSON.parse`→verify path: template parses + conforms (count 2); refs are
  all synthetic placeholders; a leak-prone extra field on the loaded file fails
  the boundary; an inflated claimed count parses but does not conform. 35→39
  tests, wired into `check:deploy`.
- Methodology doc updated to dereference the template + on-disk harness (35→39).

No promise state changed; no scale claim asserted; the template is synthetic. Still
listed: clearing the blocker still needs dropping the run's REAL per-contributor
evidence into this proven harness and citing the `ok:true` / `conforms:true`
verdict, plus owner sign-off. This run closes the "real file path is untested and
there is no document template" gap so the real-evidence run is copy-the-template,
drop-in, one call.

## Update 2026-06-20 (n) — WSL/Windows copy guard: catch verb-first over-promise phrasings

Blocker advanced this run:
`blocker.product_promises.windows_wsl_consumer_install_coverage_missing`

Updates (b)/(c) built `verifyConsumerInstallPlatformClaim` and bound it to the
real README via `auditReadmePlatformCopy` + `OVERPROMISE_COPY_PATTERNS`. But the
windows/wsl detectors required the coverage verb to appear AFTER the platform
word (`/\bwindows\b[^.\n]{0,40}\b(?:supported|in scope|works|covered)\b/i`). That
is a real false-negative: the most common drift phrasings put the verb FIRST —
"works on Windows", "runs on Windows", "supported on WSL", "we support Windows" —
and every one of those slipped through the applied guard (verified directly
against the patterns before the fix). The any-platform detector likewise only
matched the singular "any platform", missing "all/every platforms".

- `apps/pylon/src/consumer-install-platform-support.ts` — added `COVERAGE_VERB` +
  `coverageNear(platformToken)`, which builds a BIDIRECTIONAL detector (verb
  within 40 non-sentence-breaking chars before OR after the platform token), and
  rebuilt the `windows-supported-copy` / `wsl-supported-copy` patterns on it (now
  also covering `support(s)`/`run(s)`). Broadened `any-platform-copy` to
  `any|all|every|whatever` + singular/plural platform synonyms. Ref names and the
  public surface are unchanged, so the existing verifier/derived-claim flow is
  intact.
- `apps/pylon/tests/consumer-install-readme-copy-guard.test.ts` — +2 cases: five
  verb-first drift phrasings appended to the REAL README are now caught
  (`copyHonest:false`), and an honest "runs on macOS and Linux laptops" mention
  stays honest (no false positive). 31 pass (was 29) across the two copy-guard
  files.

Validation: pylon `tsc` 0 errors; workers/api `tsc` 0 errors;
`apps/openagents.com` `check:deploy` passes (see summary).

No promise state changed; no Windows/WSL support claimed; no host probed. Still
listed: clearing the blocker still needs the owner-facing copy-narrowing
sign-off. This run closes a false-negative so the applied guard actually catches
the verb-first over-promise phrasings drift most naturally takes.

## Update 2026-06-20 (m) — scale methodology: cross-contributor check no longer false-flags within-contributor repeats

Blocker advanced this run:
`blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`

Updates (d)/(g) added the cross-contributor evidence-integrity checks
(`SharedLease`/`SharedVerifiedWork`/`SharedSettlementReceipt`), but they flattened
each counted contributor's RAW ref arrays and ran `hasSharedRef` over the result.
That had a real false-negative: a single legitimate contributor whose own
evidence harmlessly lists the same ref twice (the same lease recorded from two
evidence sources, one receipt cited twice, etc.) makes the flattened array contain
a duplicate, so a real, conforming run is flagged non-conforming with a misleading
`*-across-contributors` reason. This is exactly the kind of harmless redundancy
the documented remaining step ("run the verifier against the live run's REAL
evidence") would hit, falsely failing a sound run.

- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.ts` —
  the three shared-ref checks now operate on each contributor's DISTINCT refs
  (deduped within that contributor via `flattenPerContributorDistinct`) before
  flattening. Genuine cross-contributor reuse is still caught (each sharer
  contributes one copy of the shared ref → a duplicate across the flattened set);
  harmless within-contributor repeats collapse. No reason codes, types, or public
  surface changed.
- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.test.ts`
  — +2 vitest cases (single contributor repeating its own lease/work/receipt
  conforms; two contributors that each repeat their own lease AND share it across
  each other still fail `SharedLease`). 33→35 tests, wired into `check:deploy`.
- Methodology doc updated to document the per-contributor dedup (33→35 tests).

No promise state changed; no scale claim asserted. Still listed: clearing it
needs running the verifier against the live run's REAL evidence file and citing
the `ok:true` `verdict.conforms === true`, plus owner sign-off. This run closes a
correctness gap that would have falsely failed a sound real-evidence run.

## Update 2026-06-20 (l) — scale methodology: fused safe parse→verify entry

Blocker advanced this run:
`blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`

Update (h) added `parseQualifiedContributorMethodologyInput` (the untrusted-input
parse boundary) and (a) the run-level verifier, but they stayed two SEPARATE
exported halves. The documented remaining step — "run the verifier against the
live run's real evidence" — loads an untrusted JSON document, so the correct
flow is parse → (only if ok) verify. Exposing both halves separately left a real
footgun: a caller can skip the parse boundary entirely by type-asserting the raw
document straight into `verifyQualifiedContributorMethodology`, silently
defeating the closed key allowlist and type checks the boundary exists to
enforce. Nothing made the boundary unbypassable for the real-evidence run.

- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.ts` —
  added `verifyQualifiedContributorMethodologyDocument(candidate: unknown)`: the
  single safe entry that fuses parse → verify. Returns `{ ok:false, errors }`
  with path-qualified parse reasons (verifying nothing) or `{ ok:true, verdict }`
  with the conformance verdict. Pure; counts nothing beyond the existing rule and
  asserts no scale claim. Also exported
  `QualifiedContributorMethodologyDocumentResult`.
- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.test.ts`
  — +7 vitest cases (sound/conforming; JSON round-trip; inflated-count parses but
  fails conformance; malformed fails the boundary with no `verdict`; leak-prone
  extra field rejected before any verification; non-object rejected). 26→33 tests,
  wired into `check:deploy`.
- Methodology doc updated to dereference the fused entry (26→33 tests).

No promise state changed; no scale claim asserted. Still listed: clearing it
needs running this entry against the live run's REAL evidence file and citing the
`ok:true` `verdict.conforms === true`, plus owner sign-off. This run closes the
"parse boundary is bypassable / two-step manual glue" gap so the real-evidence
run is a single, boundary-safe call.

## Update 2026-06-20 (k) — autostart receipts: fail-closed capture orchestrator

Blocker advanced this run:
`blocker.product_promises.spark_helper_autostart_receipt_missing`

Prior runs built the classifier, the receipt builder, and the single/set
verifiers — every piece a capture needs — but NO code path actually produced a
gate-valid artifact. The capture runbook's step 4 ("classify the receive
projection, then build the receipt … and write the JSON artifact") was manual
prose, so nothing guaranteed the written artifact passes
`verifySparkHelperAutostartReceipt`; only an auditor running the verifier
afterward would catch a non-conforming or leaky file. That is a real integrity
gap on the actual capture step.

- `apps/pylon/src/spark-helper-autostart.ts` — added
  `captureSparkHelperAutostartReceipt(receive, observedAt, opts)`: a pure,
  fail-closed orchestrator that composes classify → build → **self-verify** →
  canonical serialize. It returns `{ captured: true, receipt, verification,
  serialized }` ONLY when the built receipt passes its own single-receipt audit
  AND survives a JSON round-trip re-audit; otherwise `{ captured: false,
  reasons[], projection }` and emits nothing. So any artifact the self-serve path
  persists is gate-valid by construction. Also added
  `serializeSparkHelperAutostartReceipt` (canonical, fixed-key-order JSON over the
  closed allowlist, key-insertion-order independent). Writes no file; inert when
  not opted in.
- `apps/pylon/src/spark-helper-autostart.test.ts` — +6 bun:test cases (inert /
  helper-not-ready → not captured; ready → self-verified + round-trip-valid
  artifact; deterministic canonical serialization; non-canonical timestamp
  fail-closed; two distinct captures differ only by `observedAt` and pass the set
  verifier as distinct). 32 pass (was 26).
- `apps/pylon/docs/spark-helper-autostart-receipt-capture.md` — capture step 4 now
  dereferences `captureSparkHelperAutostartReceipt` (persist `result.serialized`
  verbatim only when `captured`); documented both new functions.

No promise state changed; nothing started, no funds moved, no host probed, no
file written. Still listed: clearing the blocker needs a REAL captured receipt
from ≥1 distinct normal contributor that this orchestrator produces with
`captured:true` and that passes `verifySparkHelperAutostartReceiptSet` with
`clearsBlocker:true`, plus owner sign-off. This run closes the "no code path
emits a verified artifact" gap so the capture step is fail-closed, not manual.

## Update 2026-06-20 (j) — WSL scope-out: wire it into the runtime install path

Blocker advanced this run:
`blocker.product_promises.windows_wsl_consumer_install_coverage_missing`

Update (i) added `detectWslHost` + `classifyConsumerInstallHost` but explicitly
noted the remaining gap: "the runtime install/bootstrap path does not yet WIRE
`detectWslHost` to refuse/guide a WSL contributor". That gap was real and load-
bearing — the actual `pylon bootstrap` command gated on
`summary.platform.supported`, which is `true` for a WSL host (WSL reports
`platform === "linux"`), so a WSL contributor would have been silently treated as
a supported `linux` install, directly contradicting the documented scope-out.

- `apps/pylon/src/wsl-host-detect.ts` — NEW dependency-free leaf module holding
  `detectWslHost` + `WSL_ENV_SIGNALS` (pure, public-safe; reads no files, emits no
  env value/path/identifier). Extracted so `bootstrap.ts` can share it without a
  circular import; `consumer-install-platform-support.ts` re-exports it so its
  public surface is unchanged.
- `apps/pylon/src/bootstrap.ts` — `createBootstrapSummary` now derives
  `platform.wsl` (WSL env signal on a `linux` host) and `platform.inScope`
  (`supported && !wsl`). `supported` keeps its raw-platform meaning.
- `apps/pylon/src/index.ts` — the `pylon bootstrap` command now refuses on
  `!platform.inScope` and prints WSL-specific guidance ("use a native macOS or
  Linux host") when `platform.wsl`, instead of silently proceeding.
- `apps/pylon/tests/bootstrap.test.ts` — +4 cases (WSL linux → `inScope:false`
  while `supported:true`; native linux/macOS in scope; win32 out of scope).
- `apps/pylon/docs/platform-support.md` — documented the runtime wiring.

Validation: pylon `tsc` 0 errors; `bun test tests/bootstrap.test.ts
src/consumer-install-platform-support.test.ts` → 35 pass; workers/api `tsc` 0
errors; `apps/openagents.com` `check:deploy` passes.

No promise state changed; no Windows/WSL support claimed; no host probed. Still
listed: clearing the blocker still needs the owner-facing copy-narrowing sign-off.
This run closes the classifier-vs-runtime gap so the real install path can no
longer silently admit a WSL host as supported.

## Update 2026-06-20 (i) — WSL scope-out: enforce it in code, not just prose

Blocker advanced this run:
`blocker.product_promises.windows_wsl_consumer_install_coverage_missing`

Prior runs (b, c) built the copy-drift guard and bound it to the real README, but
a structural hole remained on the WSL half of this blocker: the classifier's own
comments and `platform-support.md` claimed "the WSL Linux userland that
contributors conflate with native Linux is out-of-scope", yet the code could not
enforce that. `classifyConsumerInstallPlatform` only takes a `NodeJS.Platform`,
and **WSL reports `process.platform === "linux"`** — so `isSupportedPlatform`
returned `supported` for a WSL host, directly contradicting the documented
scope-out. The WSL scope-out was prose-only.

- `apps/pylon/src/consumer-install-platform-support.ts` — added pure, public-safe
  `detectWslHost(env, procVersion?)` (boolean over `WSL_DISTRO_NAME` /
  `WSL_INTEROP` / `WSLENV` presence and optional `/proc/version` `microsoft`/`wsl`
  text; reads no files, emits no env value/path/identifier) and
  `classifyConsumerInstallHost({ platform, wsl })`, which classifies a WSL host
  (`wsl:true` on linux) `out-of-scope` with `reason.platform.wsl_out_of_scope` and
  the blocker ref. `classifyConsumerInstallPlatform` is now a thin no-WSL wrapper
  over it (existing behavior unchanged).
- `apps/pylon/src/consumer-install-platform-support.test.ts` — +9 bun:test cases
  (WSL detection per signal, empty-value rejection, /proc/version, parity with the
  platform classifier, public-safety key audit). 23 pass (was 14).
- `apps/pylon/docs/platform-support.md` — documented the WSL detector/host
  classifier and that the WSL scope-out is now enforced in code.

No promise state changed; no Windows/WSL support claimed; no host probed; no env
value emitted. Still listed: the runtime install/bootstrap path does not yet WIRE
`detectWslHost` to refuse/guide a WSL contributor, and clearing the blocker still
needs the owner-facing copy-narrowing sign-off. This run closes the prose-vs-code
gap so a WSL host can no longer be silently classified as a supported platform.

## Update 2026-06-20 (h) — scale methodology: untrusted-input parse boundary

Blocker advanced this run:
`blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`

The verifier (`verifyQualifiedContributorMethodology`) was mature but only
consumed already-typed `QualifiedContributorEvidence[]`. The documented remaining
step — "run the verifier against the live run's REAL evidence" — implies loading
an untrusted JSON document from a file, and there was NO safe boundary for that:
a mistyped field (numeric `state`, float count, `contributors` as an object) would
silently misbehave, and a leak-prone extra field (raw address, balance, internal
id) could ride along into a published evidence artifact. The Spark side already had
this pattern (`verifySparkHelperAutostartReceipt` with a closed key allowlist); the
scale side did not.

- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.ts` —
  added `parseQualifiedContributorMethodologyInput(candidate: unknown)`: a pure
  parse/validate gate that enforces a closed key allowlist at every level
  (document, contributor, settlement receipt), checks structure/types
  (non-negative integer count; string refs; boolean receipt flags), and returns
  the typed input only when sound, else `{ ok:false, errors }` with
  path-qualified reasons (e.g. `unexpected-key:$.contributors[0].settlementReceipts[0].rawSparkAddress`).
  Also exported `QualifiedContributorMethodologyInput` and threaded it into the
  verifier signature (no behavior change).
- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.test.ts`
  — +10 vitest cases incl. JSON round-trip and leak-prone-field rejection;
  now 27 tests, wired into `check:deploy`.
- Methodology doc updated to dereference the parse boundary (17→27 tests).

No promise state changed; no scale claim asserted. Still listed: clearing it
needs running the verifier against the live run's real evidence file (now safe to
do via the parser) and citing `conforms:true`, plus owner sign-off.

## Update 2026-06-20 (g) — scale methodology: per-prong cross-contributor integrity

Blocker advanced this run:
`blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`

Update (d) closed the shared-*settlement-receipt* hole (one real Bitcoin movement
can't back two counted contributors). But two parallel integrity holes of the
EXACT same shape remained open on the other two prongs: two counted contributors
with distinct `pylonRef`s could still share the SAME admitted window lease
(prong 1) or the SAME replay-verified exact_trace work challenge (prong 2), each
inflating the qualified count just as a shared settlement inflates the real-paid
count. The promise claim rests on "two distinct *independent* contributors" who
each independently held a lease and did real verified work — so distinct pylonRefs
alone is necessary but not sufficient; the underlying evidence must be distinct too.

- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.ts` —
  the per-contributor verdict now surfaces `countedLeaseRefs` and
  `countedVerifiedWorkRefs` (the refs that satisfied prongs 1 & 2 when counting),
  alongside the existing `countedSettlementReceiptRefs`. The run-level verifier
  flattens each across counted contributors and adds
  `QualifiedRunReason.SharedLease` (`shared-lease-across-contributors`) and
  `QualifiedRunReason.SharedVerifiedWork` (`shared-verified-work-across-contributors`),
  mirroring the existing shared-settlement check via one `hasSharedRef` helper.
- `apps/openagents.com/workers/api/src/qualified-contributor-methodology.test.ts`
  — +2 vitest cases (shared lease fails; shared verified work fails), and the
  conform/verdict tests assert the new fields/reasons; now 17 tests, wired into
  `check:deploy`.
- Methodology doc updated to document cross-contributor integrity across all
  three prongs (15→17 tests).

No promise state changed; no scale claim asserted. Still listed: clearing it
needs running the verifier against the live run's real evidence and citing
`conforms:true`, plus owner sign-off.

## Update 2026-06-20 (f) — autostart receipts: reject a replicated receipt artifact

Blocker advanced this run:
`blocker.product_promises.spark_helper_autostart_receipt_missing`

Update (e) closed the reused-*contributor-ref* hole in
`verifySparkHelperAutostartReceiptSet`, but a parallel hole remained: the autostart
receipt carries no contributor binding, so an operator could capture ONE receipt on
their own host and pair byte-identical copies of it with two *distinct* fabricated
contributor refs — every entry passed (distinct refs + a valid receipt) and the set
falsely attested two distinct contributors. This is the same shape as the
`shared-settlement-receipt-across-contributors` rule already enforced in the
scale-methodology verifier (Update d), which `(e)` explicitly said it mirrored — but
the autostart set verifier did not yet enforce it.

- `apps/pylon/src/spark-helper-autostart.ts` — `verifySparkHelperAutostartReceiptSet`
  now fingerprints each valid receipt (canonical, fixed-key-order serialization over
  the closed allowlist) and rejects an exact-duplicate artifact reused across entries
  with `duplicate-receipt-artifact:<ref>`. Two genuinely independent captures differ
  at least in `observedAt`, so an exact duplicate is replication, not a second
  contributor.
- `apps/pylon/src/spark-helper-autostart.test.ts` — updated the distinct-contributors
  test to use independent (distinct-`observedAt`) receipts, and added 2 cases
  (replicated artifact under distinct refs fails; differ-only-in-`observedAt` is
  accepted as distinct). 26 pass (was 24).
- `apps/pylon/docs/spark-helper-autostart-receipt-capture.md` — documented the rule.

No promise state changed; nothing started, no funds moved, no host probed. Still
listed: clearing it needs REAL captured receipts from ≥1 distinct normal contributor
that pass `verifySparkHelperAutostartReceiptSet` with `clearsBlocker:true`, plus owner
sign-off.

## Update 2026-06-20 (e) — autostart receipts: distinct-normal-contributor set gate

Blocker advanced this run:
`blocker.product_promises.spark_helper_autostart_receipt_missing`

The prior runs built a classifier, a redacted receipt builder, and a
single-receipt verifier. But the blocker is specifically about the NORMAL
contributor self-serve path ("≥1 normal contributor reaches payout-readiness
without an operator hand-start"), and a single anonymous receipt cannot prove
that: the autostart receipt carries no contributor binding, so an operator could
capture one receipt on their own host and present it (or copies) as evidence for
"several contributors". This run adds the set-level gate that closes that hole.

- `apps/pylon/src/spark-helper-autostart.ts` — added
  `verifySparkHelperAutostartReceiptSet(entries)` over
  `{ contributorRef, receipt }[]`: audits every receipt with the existing
  single-receipt gate AND requires the contributor refs (public-safe pylonRefs)
  to be non-empty, whitespace-free, and **distinct**, rejecting a reused ref.
  Returns `{ valid, clearsBlocker, distinctContributorCount, perEntry[],
  reasons[] }`. Mirrors the scale-methodology cross-contributor integrity rule.
- `apps/pylon/src/spark-helper-autostart.test.ts` — +6 bun:test cases (24 pass).
- `apps/pylon/docs/spark-helper-autostart-receipt-capture.md` — documented the
  set-level gate.

No promise state changed; nothing started, no funds moved, no host probed. Still
listed: clearing it needs REAL captured receipts from ≥1 distinct normal
contributor that pass `verifySparkHelperAutostartReceiptSet` with
`clearsBlocker:true`, plus owner sign-off.

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
